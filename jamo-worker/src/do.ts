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

export class JamoEngine implements DurableObject {
  private state: DurableObjectState;
  private env: Env;
  private engine: WasmEngine | null = null;
  private initPromise: Promise<void> | null = null;

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

  private async ensureEngine(): Promise<WasmEngine> {
    if (this.engine) return this.engine;

    if (!this.initPromise) {
      this.initPromise = this.loadEngine();
    }
    await this.initPromise;
    return this.engine!;
  }

  private async loadEngine(): Promise<void> {
    // WASM モジュール初期化 (CF Workers では .wasm を直接 import して渡す)
    await init(wasmModule);

    // R2 から辞書バイナリをロード
    const dictObj = await this.env.DICT_BUCKET.get("ipadic.dict");
    if (!dictObj) {
      throw new Error("dictionary not found in R2 bucket");
    }

    const dictBytes = new Uint8Array(await dictObj.arrayBuffer());
    this.engine = new WasmEngine(dictBytes);
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    // WebSocket upgrade
    if (url.pathname === "/ws") {
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
          return this.handleConvert(url);
        case "/resize":
          return this.handleResize(url);
        case "/commit":
          return this.handleCommit(request);
        case "/predict":
          return this.handlePredict(url);
        case "/dict/add":
          return this.handleDictAdd(request);
        case "/dict/remove":
          return this.handleDictRemove(request);
        case "/dict/list":
          return this.handleDictList();
        case "/dict/import":
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

    const n = parseInt(url.searchParams.get("n") ?? "5", 10);
    const engine = await this.ensureEngine();
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

    const boundaries = JSON.parse(`[${boundariesParam}]`);
    const clauseIdx = parseInt(clauseParam, 10);
    const delta = parseInt(deltaParam, 10);
    const n = parseInt(url.searchParams.get("n") ?? "5", 10);

    const engine = await this.ensureEngine();
    const resultJson = engine.resize(
      q,
      JSON.stringify(boundaries),
      clauseIdx,
      delta,
      n
    );
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

    const limit = parseInt(url.searchParams.get("limit") ?? "10", 10);
    const engine = await this.ensureEngine();
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
    const body = (await request.json()) as {
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
    const body = (await request.json()) as { yomi?: string; surface?: string };
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
    const body = (await request.json()) as { yomi?: string; surface?: string };
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
    const body = (await request.json()) as {
      entries?: { yomi: string; surface: string }[];
    };
    if (!body.entries || !Array.isArray(body.entries)) {
      return jsonResponse({ error: "missing entries array" }, 400);
    }
    let added = 0;
    for (const entry of body.entries) {
      if (!entry.yomi || !entry.surface) continue;
      this.state.storage.sql.exec(
        "INSERT OR IGNORE INTO user_dict (yomi, surface) VALUES (?, ?)",
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

    try {
      const msg = JSON.parse(message);
      const id = msg.id;
      const engine = await this.ensureEngine();

      switch (msg.type) {
        case "convert": {
          const resultJson = engine.convert(msg.q ?? "", msg.n ?? 5);
          const result: ConvertResponse = JSON.parse(resultJson);
          this.injectUserDict(result);
          this.reorderCandidates(result);
          ws.send(JSON.stringify({ id, ...result }));
          break;
        }
        case "resize": {
          const resultJson = engine.resize(
            msg.q ?? "",
            JSON.stringify(msg.boundaries ?? []),
            msg.clause ?? 0,
            msg.delta ?? 0,
            msg.n ?? 5
          );
          const result: ConvertResponse = JSON.parse(resultJson);
          this.injectUserDict(result);
          this.reorderCandidates(result);
          ws.send(JSON.stringify({ id, ...result }));
          break;
        }
        case "predict": {
          const limit = msg.limit ?? 10;
          const resultJson = engine.predict(msg.q ?? "", limit);
          const result = JSON.parse(resultJson);
          // ユーザー辞書・履歴も追加
          const userCandidates: string[] = [];
          const dCursor = this.state.storage.sql.exec(
            "SELECT surface FROM user_dict WHERE yomi LIKE ? || '%' LIMIT ?",
            msg.q ?? "",
            limit
          );
          for (const row of dCursor) {
            userCandidates.push(row.surface as string);
          }
          const hCursor = this.state.storage.sql.exec(
            "SELECT surface FROM user_history WHERE yomi LIKE ? || '%' ORDER BY count DESC LIMIT ?",
            msg.q ?? "",
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
      ws.send(JSON.stringify({ error }));
    }
  }

  async webSocketClose(ws: WebSocket): Promise<void> {
    ws.close();
  }
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
