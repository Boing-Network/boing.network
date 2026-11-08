/**
 * Thin reverse proxy: Access-protected custom hostname → Fly `*.fly.dev` HTTP.
 * Public clients still use `testnet-rpc.boing.network`; this hostname is for the
 * gateway Worker (service token) and operator break-glass only.
 */

const HOP_BY_HOP = new Set([
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailers",
  "transfer-encoding",
  "upgrade",
  "cf-connecting-ip",
  "cf-ray",
  "cf-visitor",
  "cf-ipcountry",
  "x-forwarded-proto",
  "x-forwarded-for",
  "x-real-ip",
]);

function upstreamBase(env: Env): string {
  const raw = (env.UPSTREAM_ORIGIN || "https://boing-testnet-1.fly.dev").trim();
  return raw.replace(/\/+$/, "");
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const target = new URL(url.pathname + url.search, `${upstreamBase(env)}/`);

    const headers = new Headers();
    for (const [k, v] of request.headers) {
      if (HOP_BY_HOP.has(k.toLowerCase())) continue;
      // Avoid sending Access credentials to Fly.
      if (k.toLowerCase().startsWith("cf-access-")) continue;
      headers.set(k, v);
    }
    // Let fetch() derive Host from the upstream URL; do not override (breaks some TLS/SNI paths).
    headers.delete("host");
    headers.set("X-Forwarded-Host", url.host);
    headers.set("X-Boing-Origin-Proxy", "1");

    // Inject companion gate toward Fly. Only Access (custom hostname) and the gateway
    // service binding can reach this Worker once workers_dev is disabled.
    const gate =
      typeof env.BOING_ORIGIN_GATE_TOKEN === "string"
        ? env.BOING_ORIGIN_GATE_TOKEN.trim()
        : "";
    if (gate) headers.set("X-Boing-Origin-Gate", gate);

    const init: RequestInit = {
      method: request.method,
      headers,
      redirect: "manual",
    };
    if (request.method !== "GET" && request.method !== "HEAD") {
      init.body = request.body;
      // @ts-expect-error duplex required for streaming bodies in Workers
      init.duplex = "half";
    }

    try {
      return await fetch(target, init);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      return new Response(
        JSON.stringify({
          error: "origin_proxy_upstream",
          message: `Upstream fetch failed: ${message}`,
        }),
        { status: 502, headers: { "Content-Type": "application/json" } },
      );
    }
  },
};
