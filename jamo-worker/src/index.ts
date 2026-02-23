export { JamoEngine } from "./do";

interface Env {
  JAMO_ENGINE: DurableObjectNamespace;
  DICT_BUCKET: R2Bucket;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/health") {
      return new Response("ok");
    }

    // ユーザーID: Authorization ヘッダーまたは ?user= パラメータ
    const userId = getUserId(request, url);
    if (!userId) {
      return jsonResponse({ error: "missing user id" }, 401);
    }

    // Durable Object にルーティング
    const id = env.JAMO_ENGINE.idFromName(userId);
    const stub = env.JAMO_ENGINE.get(id);
    return stub.fetch(request);
  },
};

function getUserId(request: Request, url: URL): string | null {
  // クエリパラメータ (開発用)
  const userParam = url.searchParams.get("user");
  if (userParam) return userParam;

  // Authorization: Bearer <token>
  const auth = request.headers.get("Authorization");
  if (auth?.startsWith("Bearer ")) {
    // 簡易実装: トークンをそのまま user_id として使う
    // 本番では JWT デコードが必要
    return auth.slice(7);
  }

  return null;
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
