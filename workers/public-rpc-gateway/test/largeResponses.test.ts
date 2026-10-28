/**
 * Regression tests for Cloudflare error 1102 ("Worker exceeded CPU time limit", Free plan = 10 ms) on large
 * `boing_getBlockByHeight` responses (ContractDeploy blocks of 2-160 MiB). See src/index.ts
 * (UPSTREAM_ACCEPT_ENCODING, FULL_VALIDATE_MAX_BYTES, hasJsonEnvelope).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import worker, { hasJsonEnvelope } from "../src/index";

type Env = Parameters<typeof worker.fetch>[1];

const ORIGIN = "https://boing-testnet-1.fly.dev";
const env: Env = { RPC_BACKENDS: ORIGIN, EDGE_POLICY_MODE: "off" };
const MIB = 1_048_576;

let upstreamPostHeaders: Headers[] = [];
let respond: () => Response;

beforeEach(() => {
  upstreamPostHeaders = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      if (String(input) === `${ORIGIN}/live`) return new Response("ok", { status: 200 });
      upstreamPostHeaders.push(new Headers(init?.headers));
      return respond();
    })
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const getBlock = () =>
  new Request("https://testnet-rpc.boing.network/", {
    method: "POST",
    headers: { "Content-Type": "application/json", "cf-connecting-ip": "203.0.113.7" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "boing_getBlockByHeight", params: [1312, false] }),
  });

/** A JSON-RPC response with `bytes` bytes of payload, optionally advertising Content-Length. */
function bigJson(bytes: number, opts: { declare?: boolean; close?: boolean } = {}): Response {
  const { declare = true, close = true } = opts;
  const body = `{"jsonrpc":"2.0","id":1,"result":{"data":"${"a".repeat(bytes - 40)}"}${close ? "}" : ""}`;
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (declare) headers["Content-Length"] = String(new TextEncoder().encode(body).byteLength);
  return new Response(body, { status: 200, headers });
}

describe("gateway CPU budget for large upstream responses", () => {
  it("asks the node for an identity-encoded body so it can see Content-Length", async () => {
    respond = () => new Response(JSON.stringify({ jsonrpc: "2.0", id: 1, result: 1322 }), { status: 200 });
    const res = await worker.fetch(getBlock(), env);
    expect(res.status).toBe(200);
    expect(upstreamPostHeaders[0]?.get("accept-encoding")).toBe("identity");
  });

  it("rejects a declared >6 MiB response without reading its body", async () => {
    let pulls = 0;
    let cancelled = false;
    const stream = new ReadableStream<Uint8Array>({
      pull(controller) {
        pulls += 1;
        controller.enqueue(new Uint8Array(64 * 1024).fill(0x61));
      },
      cancel() {
        cancelled = true;
      },
    }, { highWaterMark: 0 });
    respond = () =>
      new Response(stream, { status: 200, headers: { "Content-Type": "application/json", "Content-Length": String(167_773_141) } });
    const res = await worker.fetch(getBlock(), env);
    const body = (await res.json()) as { error: { code: number; message: string } };
    expect(res.status).toBe(200);
    expect(body.error.code).toBe(-32000);
    expect(body.error.message).toMatch(/too large/i);
    expect(pulls).toBe(0);
    expect(cancelled).toBe(true);
  });

  it("forwards a valid 2 MiB response intact without JSON.parse-ing the body", async () => {
    respond = () => bigJson(2 * MIB);
    const parse = vi.spyOn(JSON, "parse");
    const res = await worker.fetch(getBlock(), env);
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text.length).toBeGreaterThan(2 * MIB - 100);
    expect(text.endsWith("}}")).toBe(true);
    // Only the tiny request body may be parsed; never anything near the response size.
    for (const call of parse.mock.calls) expect(String(call[0]).length).toBeLessThan(1024);
  });

  it("rejects a large body shorter than its declared Content-Length", async () => {
    respond = () => {
      const full = bigJson(2 * MIB);
      const headers = new Headers(full.headers);
      headers.set("Content-Length", String(2 * MIB + 5_000));
      return new Response(full.body, { status: 200, headers });
    };
    const res = await worker.fetch(getBlock(), env);
    const body = (await res.json()) as { error: { message: string } };
    expect(body.error.message).toMatch(/truncated/);
  });

  it("still fully parses a large body that has no declared length (cut-off is rejected)", async () => {
    respond = () => bigJson(2 * MIB, { declare: false, close: false });
    const res = await worker.fetch(getBlock(), env);
    const body = (await res.json()) as { error: { message: string } };
    expect(body.error.message).toMatch(/truncated or invalid JSON/);
  });

  it("still fully validates small bodies", async () => {
    respond = () => new Response('{"jsonrpc":"2.0","id":1,"result":', { status: 200 });
    const res = await worker.fetch(getBlock(), env);
    const body = (await res.json()) as { error: { message: string } };
    expect(body.error.message).toMatch(/truncated or invalid JSON/);
  });
});

describe("hasJsonEnvelope", () => {
  const enc = (s: string) => new TextEncoder().encode(s);
  it("accepts objects and arrays, ignoring surrounding whitespace", () => {
    expect(hasJsonEnvelope(enc('{"a":1}'))).toBe(true);
    expect(hasJsonEnvelope(enc(' \n[1,2]\r\n'))).toBe(true);
  });
  it("rejects truncated, empty and non-container bodies", () => {
    expect(hasJsonEnvelope(enc('{"a":[1,2'))).toBe(false);
    expect(hasJsonEnvelope(enc('{"a":1]'))).toBe(false);
    expect(hasJsonEnvelope(enc(""))).toBe(false);
    expect(hasJsonEnvelope(enc("   "))).toBe(false);
    expect(hasJsonEnvelope(enc("<html></html>"))).toBe(false);
    expect(hasJsonEnvelope(enc("{"))).toBe(false);
  });
});
