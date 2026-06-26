export { JamoEngine } from "./do";

interface Env {
  JAMO_ENGINE: DurableObjectNamespace;
  DICT_BUCKET: R2Bucket;
  JWT_SECRET?: string;
  ALLOW_DEV_USER?: string;
  ALLOWED_ORIGINS?: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") {
      return corsPreflight(request, env);
    }

    if (url.pathname === "/health") {
      return withCors(new Response("ok"), request, env);
    }

    // ユーザーID: Authorization ヘッダーまたは ?user= パラメータ
    const userId = await getUserId(request, url, env);
    if (!userId) {
      return withCors(jsonResponse({ error: "unauthorized" }, 401), request, env);
    }

    // Durable Object にルーティング
    const id = env.JAMO_ENGINE.idFromName(userId);
    const stub = env.JAMO_ENGINE.get(id);
    const response = await stub.fetch(request);
    if (request.headers.get("Upgrade") === "websocket") {
      return response;
    }
    return withCors(response, request, env);
  },
};

async function getUserId(
  request: Request,
  url: URL,
  env: Env
): Promise<string | null> {
  // クエリパラメータ (開発用)
  const userParam = url.searchParams.get("user");
  if (userParam && env.ALLOW_DEV_USER === "true") return userParam;

  // Authorization: Bearer <JWT>
  const auth = request.headers.get("Authorization");
  if (auth?.startsWith("Bearer ")) {
    return verifyJwtAndGetSubject(auth.slice(7), env.JWT_SECRET);
  }

  return null;
}

async function verifyJwtAndGetSubject(
  token: string,
  secret?: string
): Promise<string | null> {
  if (!secret) return null;

  const parts = token.split(".");
  if (parts.length !== 3) return null;

  let header: { alg?: string; typ?: string };
  let payload: { sub?: unknown; exp?: unknown; nbf?: unknown };
  try {
    header = JSON.parse(decodeBase64UrlToString(parts[0]));
    payload = JSON.parse(decodeBase64UrlToString(parts[1]));
  } catch {
    return null;
  }

  if (header.alg !== "HS256") return null;

  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"]
  );
  const signature = decodeBase64Url(parts[2]);
  const valid = await crypto.subtle.verify(
    "HMAC",
    key,
    signature,
    new TextEncoder().encode(`${parts[0]}.${parts[1]}`)
  );
  if (!valid) return null;

  const now = Math.floor(Date.now() / 1000);
  if (typeof payload.exp === "number" && payload.exp <= now) return null;
  if (typeof payload.nbf === "number" && payload.nbf > now) return null;
  return typeof payload.sub === "string" && payload.sub.length > 0
    ? payload.sub
    : null;
}

function decodeBase64UrlToString(value: string): string {
  return new TextDecoder().decode(decodeBase64Url(value));
}

function decodeBase64Url(value: string): Uint8Array {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(
    Math.ceil(value.length / 4) * 4,
    "="
  );
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function corsPreflight(request: Request, env: Env): Response {
  const origin = allowedOrigin(request, env);
  if (!origin) {
    return jsonResponse({ error: "origin not allowed" }, 403);
  }
  return new Response(null, {
    status: 204,
    headers: corsHeaders(origin),
  });
}

function withCors(response: Response, request: Request, env: Env): Response {
  const origin = allowedOrigin(request, env);
  if (!origin) return response;

  const headers = new Headers(response.headers);
  for (const [key, value] of corsHeaders(origin)) {
    headers.set(key, value);
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function allowedOrigin(request: Request, env: Env): string | null {
  const origin = request.headers.get("Origin");
  if (!origin || !env.ALLOWED_ORIGINS) return null;
  const allowed = env.ALLOWED_ORIGINS.split(",").map((s) => s.trim());
  return allowed.includes(origin) ? origin : null;
}

function corsHeaders(origin: string): Headers {
  return new Headers({
    "Access-Control-Allow-Origin": origin,
    "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
    "Access-Control-Allow-Headers": "Authorization,Content-Type",
    Vary: "Origin",
  });
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
