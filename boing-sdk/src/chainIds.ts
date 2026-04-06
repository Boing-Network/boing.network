/**
 * Chain identifiers for **native Boing** L1 — use in multi-network dApps (deploy wizards, network pickers).
 *
 * Prefer **`boing_getNetworkInfo`**.`chain_id` when connected to a live RPC; use these constants when
 * branching on **`eth_chainId`** / **`boing_chainId`** from the wallet.
 */

/** Public testnet chain id (decimal), per docs ([TESTNET.md](../../docs/TESTNET.md)). */
export const BOING_TESTNET_CHAIN_ID_DECIMAL = 6913;

/** Same id as EIP-155-style `0x`-prefixed hex (lowercase). */
export const BOING_TESTNET_CHAIN_ID_HEX = '0x1b01' as const;

function strip0x(h: string): string {
  const t = h.trim().toLowerCase();
  return t.startsWith('0x') ? t.slice(2) : t;
}

/**
 * Normalize wallet/RPC chain id to lowercase `0x` hex (e.g. **`0x1b01`**).
 * Accepts decimal string, `0x` hex, or bigint / number.
 */
export function normalizeBoingChainIdHex(chainId: string | number | bigint): `0x${string}` {
  if (typeof chainId === 'bigint') {
    if (chainId < 0n) {
      throw new Error('normalizeBoingChainIdHex: negative chain id');
    }
    return `0x${chainId.toString(16)}` as `0x${string}`;
  }
  if (typeof chainId === 'number') {
    if (!Number.isInteger(chainId) || chainId < 0) {
      throw new Error(`normalizeBoingChainIdHex: invalid number ${chainId}`);
    }
    return `0x${BigInt(chainId).toString(16)}` as `0x${string}`;
  }
  const s = chainId.trim();
  if (/^\d+$/.test(s)) {
    const n = BigInt(s);
    if (n < 0n) {
      throw new Error('normalizeBoingChainIdHex: negative chain id');
    }
    return `0x${n.toString(16)}` as `0x${string}`;
  }
  const bare = strip0x(s);
  if (!/^[0-9a-f]+$/i.test(bare)) {
    throw new Error('normalizeBoingChainIdHex: expected decimal or hex chain id');
  }
  const n = BigInt(`0x${bare}`);
  return `0x${n.toString(16)}` as `0x${string}`;
}

/** True when **`chainId`** is **Boing testnet** (6913 / `0x1b01`), after normalization. */
export function isBoingTestnetChainId(chainId: string | number | bigint): boolean {
  try {
    const h = normalizeBoingChainIdHex(chainId).toLowerCase();
    return h === BOING_TESTNET_CHAIN_ID_HEX;
  } catch {
    return false;
  }
}
