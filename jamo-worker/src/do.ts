import init, { JamoEngine as WasmEngine } from "../../jamo-core/pkg/jamo_core";
import wasmModule from "../../jamo-core/pkg/jamo_core_bg.wasm";

interface Env {
  DICT_BUCKET: R2Bucket;
}

interface Clause {
  yomi: string;
  candidates: string[];
}

interface ConvertResponse {
  result: Clause[];
  boundaries: number[];
}

const MAX_HISTORY = 10000;

const DEFAULT_DOMAIN = "ipadic";

export class JamoEngine implements DurableObject {
  private state: DurableObjectState;
  private env: Env;
  private engines: Map<string, WasmEngine> = new Map();
  private loadPromises: Map<string, Promise<void>> = new Map();
  private wasmInitialized = false;

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.env = env;
    this.initDb();
  }

  private initDb(): void {
    this.state.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS user_history (
        yomi TEXT NOT NULL,
        surface TEXT NOT NULL,
        count INTEGER NOT NULL DEFAULT 1,
        last_used INTEGER NOT NULL,
        PRIMARY KEY (yomi, surface)
      )
    `);
    this.state.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS user_dict (
        yomi TEXT NOT NULL,
        surface TEXT NOT NULL,
        PRIMARY KEY (yomi, surface)
      )
    `);
  }

  private async ensureEngine(domain?: string): Promise<WasmEngine> {
    const key = domain || DEFAULT_DOMAIN;
    const cached = this.engines.get(key);
    if (cached) return cached;

    if (!this.loadPromises.has(key)) {
      const promise = this.loadEngine(key).finally(() => {
        this.loadPromises.delete(key);
      });
      this.loadPromises.set(key, promise);
    }
    await this.loadPromises.get(key)!;
    return this.engines.get(key)!;
  }

  private async loadEngine(domain: string): Promise<void> {
    // WASM モジュール初期化 (1回のみ)
    if (!this.wasmInitialized) {
      await init(wasmModule);
      this.wasmInitialized = true;
    }

    // R2 から辞書バイナリをロード
    const dictKey = `${domain}.dict`;
    const dictObj = await this.env.DICT_BUCKET.get(dictKey);
    if (!dictObj) {
      throw new Error(`dictionary '${dictKey}' not found in R2 bucket`);
    }

    const dictBytes = new Uint8Array(await dictObj.arrayBuffer());
    this.engines.set(domain, new WasmEngine(dictBytes));
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    // WebSocket upgrade
    if (url.pathname === "/ws") {
      if (request.method !== "GET") {
        return methodNotAllowed();
      }
      if (request.headers.get("Upgrade") !== "websocket") {
        return jsonResponse({ error: "expected websocket" }, 426);
      }
      const pair = new WebSocketPair();
      this.state.acceptWebSocket(pair[1]);
      return new Response(null, { status: 101, webSocket: pair[0] });
    }

    try {
      switch (url.pathname) {
        case "/convert":
          if (request.method !== "GET") return methodNotAllowed();
          return this.handleConvert(url);
        case "/resize":
          if (request.method !== "GET") return methodNotAllowed();
          return this.handleResize(url);
        case "/commit":
          if (request.method !== "POST") return methodNotAllowed();
          return this.handleCommit(request);
        case "/predict":
          if (request.method !== "GET") return methodNotAllowed();
          return this.handlePredict(url);
        case "/dict/add":
          if (request.method !== "POST") return methodNotAllowed();
          return this.handleDictAdd(request);
        case "/dict/remove":
          if (request.method !== "POST") return methodNotAllowed();
          return this.handleDictRemove(request);
        case "/dict/list":
          if (request.method !== "GET") return methodNotAllowed();
          return this.handleDictList();
        case "/dict/import":
          if (request.method !== "POST") return methodNotAllowed();
          return this.handleDictImport(request);
        default:
          return jsonResponse({ error: "not found" }, 404);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 500);
    }
  }

  private async handleConvert(url: URL): Promise<Response> {
    const q = url.searchParams.get("q");
    if (!q) {
      return jsonResponse({ error: "missing q parameter" }, 400);
    }

    let n: number;
    try {
      n = parseNonNegativeInt(url.searchParams.get("n") ?? "5", "n");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 400);
    }
    const domain = url.searchParams.get("domain") ?? undefined;
    const engine = await this.ensureEngine(domain);
    const resultJson = engine.convert(q, n);
    const result: ConvertResponse = JSON.parse(resultJson);
    this.injectUserDict(result);
    this.reorderCandidates(result);
    return new Response(JSON.stringify(result), {
      headers: { "Content-Type": "application/json" },
    });
  }

  private async handleResize(url: URL): Promise<Response> {
    const q = url.searchParams.get("q");
    const boundariesParam = url.searchParams.get("boundaries");
    const clauseParam = url.searchParams.get("clause");
    const deltaParam = url.searchParams.get("delta");

    if (!q || !boundariesParam || !clauseParam || !deltaParam) {
      return jsonResponse(
        { error: "missing required parameters (q, boundaries, clause, delta)" },
        400
      );
    }

    let boundaries: number[];
    try {
      boundaries = parseBoundariesParam(boundariesParam);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 400);
    }
    let clauseIdx: number;
    let delta: number;
    let n: number;
    try {
      clauseIdx = parseNonNegativeInt(clauseParam, "clause");
      delta = parseInteger(deltaParam, "delta");
      n = parseNonNegativeInt(url.searchParams.get("n") ?? "5", "n");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 400);
    }
    const domain = url.searchParams.get("domain") ?? undefined;

    const engine = await this.ensureEngine(domain);
    let resultJson: string;
    try {
      resultJson = engine.resize(q, JSON.stringify(boundaries), clauseIdx, delta, n);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 400);
    }
    const result: ConvertResponse = JSON.parse(resultJson);
    this.injectUserDict(result);
    this.reorderCandidates(result);
    return new Response(JSON.stringify(result), {
      headers: { "Content-Type": "application/json" },
    });
  }

  private async handlePredict(url: URL): Promise<Response> {
    const q = url.searchParams.get("q");
    if (!q) {
      return jsonResponse({ error: "missing q parameter" }, 400);
    }

    let limit: number;
    try {
      limit = parseNonNegativeInt(url.searchParams.get("limit") ?? "10", "limit");
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 400);
    }
    const domain = url.searchParams.get("domain") ?? undefined;
    const engine = await this.ensureEngine(domain);
    const resultJson = engine.predict(q, limit);
    const result = JSON.parse(resultJson);

    // ユーザー辞書・履歴からも予測候補を追加
    const userCandidates: string[] = [];
    const dictCursor = this.state.storage.sql.exec(
      "SELECT surface FROM user_dict WHERE yomi LIKE ? || '%' LIMIT ?",
      q,
      limit
    );
    for (const row of dictCursor) {
      userCandidates.push(row.surface as string);
    }
    const historyCursor = this.state.storage.sql.exec(
      "SELECT surface FROM user_history WHERE yomi LIKE ? || '%' ORDER BY count DESC, last_used DESC LIMIT ?",
      q,
      limit
    );
    for (const row of historyCursor) {
      const s = row.surface as string;
      if (!userCandidates.includes(s)) {
        userCandidates.push(s);
      }
    }

    // ユーザー候補を先頭に挿入
    const existing = new Set(
      (result.candidates as { surface: string }[]).map((c) => c.surface)
    );
    const merged = [
      ...userCandidates
        .filter((s) => !existing.has(s))
        .map((s) => ({ surface: s, cost: 0 })),
      ...result.candidates,
    ];

    return jsonResponse({ candidates: merged.slice(0, limit) });
  }

  private async handleCommit(request: Request): Promise<Response> {
    const parsed = await readJson(request);
    if (!parsed.ok) return parsed.response;

    const body = parsed.value as {
      clauses?: { yomi: string; surface: string }[];
    };
    if (!body.clauses || !Array.isArray(body.clauses)) {
      return jsonResponse({ error: "missing clauses array" }, 400);
    }

    const now = Date.now();
    for (const clause of body.clauses) {
      if (!clause.yomi || !clause.surface) continue;
      this.state.storage.sql.exec(
        `INSERT INTO user_history (yomi, surface, count, last_used)
         VALUES (?, ?, 1, ?)
         ON CONFLICT (yomi, surface)
         DO UPDATE SET count = count + 1, last_used = ?`,
        clause.yomi,
        clause.surface,
        now,
        now
      );
    }

    // LRU: 古いエントリを削除
    this.state.storage.sql.exec(
      `DELETE FROM user_history WHERE rowid NOT IN (
         SELECT rowid FROM user_history ORDER BY last_used DESC LIMIT ?
       )`,
      MAX_HISTORY
    );

    return jsonResponse({ ok: true });
  }

  private async handleDictAdd(request: Request): Promise<Response> {
    const parsed = await readJson(request);
    if (!parsed.ok) return parsed.response;

    const body = parsed.value as { yomi?: string; surface?: string };
    if (!body.yomi || !body.surface) {
      return jsonResponse({ error: "missing yomi or surface" }, 400);
    }
    this.state.storage.sql.exec(
      "INSERT OR IGNORE INTO user_dict (yomi, surface) VALUES (?, ?)",
      body.yomi,
      body.surface
    );
    return jsonResponse({ ok: true });
  }

  private async handleDictRemove(request: Request): Promise<Response> {
    const parsed = await readJson(request);
    if (!parsed.ok) return parsed.response;

    const body = parsed.value as { yomi?: string; surface?: string };
    if (!body.yomi || !body.surface) {
      return jsonResponse({ error: "missing yomi or surface" }, 400);
    }
    this.state.storage.sql.exec(
      "DELETE FROM user_dict WHERE yomi = ? AND surface = ?",
      body.yomi,
      body.surface
    );
    return jsonResponse({ ok: true });
  }

  private handleDictList(): Response {
    const cursor = this.state.storage.sql.exec(
      "SELECT yomi, surface FROM user_dict ORDER BY yomi"
    );
    const entries = [];
    for (const row of cursor) {
      entries.push({ yomi: row.yomi, surface: row.surface });
    }
    return jsonResponse({ entries });
  }

  private async handleDictImport(request: Request): Promise<Response> {
    const parsed = await readJson(request);
    if (!parsed.ok) return parsed.response;

    const body = parsed.value as {
      entries?: { yomi: string; surface: string }[];
    };
    if (!body.entries || !Array.isArray(body.entries)) {
      return jsonResponse({ error: "missing entries array" }, 400);
    }
    let added = 0;
    for (const entry of body.entries) {
      if (!entry.yomi || !entry.surface) continue;
      let exists = false;
      const existing = this.state.storage.sql.exec(
        "SELECT 1 FROM user_dict WHERE yomi = ? AND surface = ? LIMIT 1",
        entry.yomi,
        entry.surface
      );
      for (const _row of existing) {
        exists = true;
        break;
      }
      if (exists) continue;
      this.state.storage.sql.exec(
        "INSERT INTO user_dict (yomi, surface) VALUES (?, ?)",
        entry.yomi,
        entry.surface
      );
      added++;
    }
    return jsonResponse({ ok: true, added });
  }

  private injectUserDict(result: ConvertResponse): void {
    for (const clause of result.result) {
      const cursor = this.state.storage.sql.exec(
        "SELECT surface FROM user_dict WHERE yomi = ?",
        clause.yomi
      );
      for (const row of cursor) {
        const surface = row.surface as string;
        if (!clause.candidates.includes(surface)) {
          clause.candidates.unshift(surface);
        }
      }
    }
  }

  private reorderCandidates(result: ConvertResponse): void {
    for (const clause of result.result) {
      if (clause.candidates.length <= 1) continue;

      const cursor = this.state.storage.sql.exec(
        "SELECT surface, count FROM user_history WHERE yomi = ? ORDER BY count DESC, last_used DESC",
        clause.yomi
      );
      const learned = new Map<string, number>();
      for (const row of cursor) {
        learned.set(row.surface as string, row.count as number);
      }
      if (learned.size === 0) continue;

      clause.candidates.sort((a, b) => {
        const ca = learned.get(a) ?? 0;
        const cb = learned.get(b) ?? 0;
        if (ca !== cb) return cb - ca;
        return 0; // 学習データがない場合は元の順序を維持
      });
    }
  }

  async webSocketMessage(
    ws: WebSocket,
    message: string | ArrayBuffer
  ): Promise<void> {
    if (typeof message !== "string") {
      ws.send(JSON.stringify({ error: "binary messages not supported" }));
      return;
    }

    let id: unknown;
    try {
      const msg = JSON.parse(message);
      id = msg.id;

      switch (msg.type) {
        case "convert": {
          const q = requireString(msg.q, "q");
          const n = parseOptionalNonNegativeInt(msg.n, 5, "n");
          const engine = await this.ensureEngine(msg.domain);
          const resultJson = engine.convert(q, n);
          const result: ConvertResponse = JSON.parse(resultJson);
          this.injectUserDict(result);
          this.reorderCandidates(result);
          ws.send(JSON.stringify({ id, ...result }));
          break;
        }
        case "resize": {
          const q = requireString(msg.q, "q");
          const boundaries = parseBoundariesValue(msg.boundaries);
          const clause = parseNonNegativeIntegerValue(msg.clause, "clause");
          const delta = parseIntegerValue(msg.delta, "delta");
          const n = parseOptionalNonNegativeInt(msg.n, 5, "n");
          const engine = await this.ensureEngine(msg.domain);
          const resultJson = engine.resize(
            q,
            JSON.stringify(boundaries),
            clause,
            delta,
            n
          );
          const result: ConvertResponse = JSON.parse(resultJson);
          this.injectUserDict(result);
          this.reorderCandidates(result);
          ws.send(JSON.stringify({ id, ...result }));
          break;
        }
        case "predict": {
          const q = requireString(msg.q, "q");
          const limit = parseOptionalNonNegativeInt(msg.limit, 10, "limit");
          const engine = await this.ensureEngine(msg.domain);
          const resultJson = engine.predict(q, limit);
          const result = JSON.parse(resultJson);
          // ユーザー辞書・履歴も追加
          const userCandidates: string[] = [];
          const dCursor = this.state.storage.sql.exec(
            "SELECT surface FROM user_dict WHERE yomi LIKE ? || '%' LIMIT ?",
            q,
            limit
          );
          for (const row of dCursor) {
            userCandidates.push(row.surface as string);
          }
          const hCursor = this.state.storage.sql.exec(
            "SELECT surface FROM user_history WHERE yomi LIKE ? || '%' ORDER BY count DESC LIMIT ?",
            q,
            limit
          );
          for (const row of hCursor) {
            const s = row.surface as string;
            if (!userCandidates.includes(s)) userCandidates.push(s);
          }
          const existing = new Set(
            (result.candidates as { surface: string }[]).map(
              (c: { surface: string }) => c.surface
            )
          );
          const merged = [
            ...userCandidates
              .filter((s: string) => !existing.has(s))
              .map((s: string) => ({ surface: s, cost: 0 })),
            ...result.candidates,
          ];
          ws.send(
            JSON.stringify({
              id,
              candidates: merged.slice(0, limit),
            })
          );
          break;
        }
        case "commit": {
          const now = Date.now();
          for (const clause of msg.clauses ?? []) {
            if (!clause.yomi || !clause.surface) continue;
            this.state.storage.sql.exec(
              `INSERT INTO user_history (yomi, surface, count, last_used)
               VALUES (?, ?, 1, ?)
               ON CONFLICT (yomi, surface)
               DO UPDATE SET count = count + 1, last_used = ?`,
              clause.yomi,
              clause.surface,
              now,
              now
            );
          }
          this.state.storage.sql.exec(
            `DELETE FROM user_history WHERE rowid NOT IN (
               SELECT rowid FROM user_history ORDER BY last_used DESC LIMIT ?
             )`,
            MAX_HISTORY
          );
          ws.send(JSON.stringify({ id, ok: true }));
          break;
        }
        case "dict.add": {
          if (msg.yomi && msg.surface) {
            this.state.storage.sql.exec(
              "INSERT OR IGNORE INTO user_dict (yomi, surface) VALUES (?, ?)",
              msg.yomi,
              msg.surface
            );
          }
          ws.send(JSON.stringify({ id, ok: true }));
          break;
        }
        case "dict.remove": {
          if (msg.yomi && msg.surface) {
            this.state.storage.sql.exec(
              "DELETE FROM user_dict WHERE yomi = ? AND surface = ?",
              msg.yomi,
              msg.surface
            );
          }
          ws.send(JSON.stringify({ id, ok: true }));
          break;
        }
        case "dict.list": {
          const cursor = this.state.storage.sql.exec(
            "SELECT yomi, surface FROM user_dict ORDER BY yomi"
          );
          const entries = [];
          for (const row of cursor) {
            entries.push({ yomi: row.yomi, surface: row.surface });
          }
          ws.send(JSON.stringify({ id, entries }));
          break;
        }
        default:
          ws.send(JSON.stringify({ id, error: "unknown type" }));
      }
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e);
      ws.send(JSON.stringify({ id, error }));
    }
  }

  async webSocketClose(ws: WebSocket): Promise<void> {
    ws.close();
  }
}

function parseBoundariesParam(value: string): number[] {
  if (value.trim() === "") {
    throw new Error("boundaries must not be empty");
  }
  return value.split(",").map((part) => {
    const trimmed = part.trim();
    if (trimmed === "") {
      throw new Error("empty boundary value");
    }
    const parsed = Number(trimmed);
    if (!Number.isSafeInteger(parsed) || parsed < 0) {
      throw new Error(`invalid boundary value: ${trimmed}`);
    }
    return parsed;
  });
}

function parseBoundariesValue(value: unknown): number[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error("boundaries must be a non-empty array");
  }
  return value.map((entry) => parseNonNegativeIntegerValue(entry, "boundary"));
}

function parseNonNegativeInt(value: string, name: string): number {
  const parsed = parseInteger(value, name);
  if (parsed < 0) {
    throw new Error(`${name} must be non-negative`);
  }
  return parsed;
}

function parseInteger(value: string, name: string): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${name} must be an integer`);
  }
  return parsed;
}

function parseOptionalNonNegativeInt(
  value: unknown,
  fallback: number,
  name: string
): number {
  if (value === undefined || value === null) return fallback;
  return parseNonNegativeIntegerValue(value, name);
}

function parseNonNegativeIntegerValue(value: unknown, name: string): number {
  const parsed = parseIntegerValue(value, name);
  if (parsed < 0) {
    throw new Error(`${name} must be non-negative`);
  }
  return parsed;
}

function parseIntegerValue(value: unknown, name: string): number {
  if (!Number.isSafeInteger(value)) {
    throw new Error(`${name} must be an integer`);
  }
  return value as number;
}

function requireString(value: unknown, name: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${name} must be a non-empty string`);
  }
  return value;
}

async function readJson(
  request: Request
): Promise<
  | { ok: true; value: unknown }
  | { ok: false; response: Response }
> {
  try {
    return { ok: true, value: await request.json() };
  } catch {
    return {
      ok: false,
      response: jsonResponse({ error: "invalid JSON body" }, 400),
    };
  }
}

function methodNotAllowed(): Response {
  return jsonResponse({ error: "method not allowed" }, 405);
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
