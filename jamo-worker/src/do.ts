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
