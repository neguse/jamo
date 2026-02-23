import init, { JamoEngine as WasmEngine } from "../../jamo-core/pkg/jamo_core";
import wasmModule from "../../jamo-core/pkg/jamo_core_bg.wasm";

interface Env {
  DICT_BUCKET: R2Bucket;
}

export class JamoEngine implements DurableObject {
  private state: DurableObjectState;
  private env: Env;
  private engine: WasmEngine | null = null;
  private initPromise: Promise<void> | null = null;

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.env = env;
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
        default:
          return jsonResponse({ error: "not found" }, 404);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      return jsonResponse({ error: message }, 500);
    }
  }

  async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): Promise<void> {
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
          ws.send(JSON.stringify({ id, ...JSON.parse(resultJson) }));
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
          ws.send(JSON.stringify({ id, ...JSON.parse(resultJson) }));
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

  private async handleConvert(url: URL): Promise<Response> {
    const q = url.searchParams.get("q");
    if (!q) {
      return jsonResponse({ error: "missing q parameter" }, 400);
    }

    const n = parseInt(url.searchParams.get("n") ?? "5", 10);
    const engine = await this.ensureEngine();
    const resultJson = engine.convert(q, n);
    return new Response(resultJson, {
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
    return new Response(resultJson, {
      headers: { "Content-Type": "application/json" },
    });
  }
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
