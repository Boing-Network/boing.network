/**
 * client_hints defaults must not drift (docs/RPC-API-SPEC.md, "client_hints"):
 *   rpc.rs defaults == docs/chain-metadata.json == RPC-API-SPEC.md examples == ENVIRONMENT-VARIABLES.md,
 *   recommended_sdk == boing-sdk/package.json, and no hint may point at a release that does not exist.
 *
 * "Does not exist" is checked offline against what this checkout can prove (the SDK package version and,
 * when a sibling boing.express checkout is present, its extension manifest). The registry/GitHub check
 * lives in scripts/check-client-hints-published.mjs (run it before changing a default).
 */
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');
const read = (rel: string) => readFileSync(join(root, rel), 'utf8');

const rpcRs = read('crates/boing-node/src/rpc.rs');
function rustConst(name: string): string {
  const m = new RegExp(`const ${name}: &str = "([^"]+)";`).exec(rpcRs);
  if (!m) throw new Error(`${name} not found in rpc.rs`);
  return m[1];
}
const defaults = {
  min_wallet_version: rustConst('DEFAULT_MIN_WALLET_VERSION'),
  recommended_wallet_version: rustConst('DEFAULT_RECOMMENDED_WALLET_VERSION'),
  recommended_sdk: rustConst('DEFAULT_RECOMMENDED_SDK_VERSION'),
};

const semver = /^\d+\.\d+\.\d+$/;
function cmp(a: string, b: string): number {
  const x = a.split('.').map(Number);
  const y = b.split('.').map(Number);
  for (let i = 0; i < 3; i += 1) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1;
  return 0;
}

describe('client_hints defaults', () => {
  it('are plain semver and ordered (min <= recommended wallet)', () => {
    for (const v of Object.values(defaults)) expect(v).toMatch(semver);
    expect(cmp(defaults.min_wallet_version, defaults.recommended_wallet_version)).toBeLessThanOrEqual(0);
  });

  it('recommended_sdk equals boing-sdk/package.json version', () => {
    const pkg = JSON.parse(read('boing-sdk/package.json')) as { version: string; name: string };
    expect(pkg.name).toBe('@boing-network/boing-sdk');
    expect(defaults.recommended_sdk).toBe(pkg.version);
  });

  it('match docs/chain-metadata.json client_hints_defaults', () => {
    const meta = JSON.parse(read('docs/chain-metadata.json')) as {
      discovery: { client_hints_defaults: Record<string, string> };
    };
    const d = meta.discovery.client_hints_defaults;
    expect(d.min_wallet_version).toBe(defaults.min_wallet_version);
    expect(d.recommended_wallet_version).toBe(defaults.recommended_wallet_version);
    expect(d.recommended_sdk).toBe(defaults.recommended_sdk);
    expect(d.sdk_npm_package).toBe('@boing-network/boing-sdk');
  });

  it('are shown verbatim in both RPC-API-SPEC.md examples and ENVIRONMENT-VARIABLES.md', () => {
    const spec = read('docs/RPC-API-SPEC.md');
    for (const [k, v] of Object.entries(defaults)) {
      const line = `"${k}": "${v}"`;
      expect(spec.split(line).length - 1, `${line} in RPC-API-SPEC.md`).toBeGreaterThanOrEqual(2);
    }
    const env = read('docs/ENVIRONMENT-VARIABLES.md');
    expect(env).toContain(`| \`BOING_MIN_WALLET_VERSION\` | \`${defaults.min_wallet_version}\``);
    expect(env).toContain(`| \`BOING_RECOMMENDED_WALLET_VERSION\` | \`${defaults.recommended_wallet_version}\``);
    expect(env).toContain(`| \`BOING_RECOMMENDED_SDK_VERSION\` | \`${defaults.recommended_sdk}\``);
  });

  it('developer_api.json describes the same min_wallet_version default', () => {
    expect(read('crates/boing-node/schemas/developer_api.json')).toContain(`default ${defaults.min_wallet_version}`);
  });

  it('do not reference a wallet release newer than the sibling boing.express checkout (when present)', () => {
    const manifest = join(root, '..', 'boing.express', 'extension', 'manifest.json');
    if (!existsSync(manifest)) return; // CI checkout of this repo alone: covered by check-client-hints-published.mjs
    const { version } = JSON.parse(readFileSync(manifest, 'utf8')) as { version: string };
    expect(cmp(defaults.recommended_wallet_version, version), `recommended_wallet_version ${defaults.recommended_wallet_version} > extension manifest ${version}`).toBeLessThanOrEqual(0);
    expect(cmp(defaults.min_wallet_version, version)).toBeLessThanOrEqual(0);
  });
});