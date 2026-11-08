interface Env {
  UPSTREAM_ORIGIN: string;
  /** Optional; forwarded as X-Boing-Origin-Gate to the Fly node companion gate. */
  BOING_ORIGIN_GATE_TOKEN?: string;
}

declare module "cloudflare:workers" {
  // Empty: Env is declared above for this Worker.
}
