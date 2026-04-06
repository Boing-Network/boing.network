/**
 * Thin **EIP-1193** helpers for **Boing Express** and compatible injected providers.
 * Reduces branching in dApps that call **`boing_sendTransaction`**, **`boing_requestAccounts`**, **`boing_chainId`**.
 */

const BOING_SEND = 'boing_sendTransaction';
const BOING_ACCOUNTS = 'boing_requestAccounts';
const BOING_CHAIN = 'boing_chainId';
const ETH_CHAIN = 'eth_chainId';
const ETH_ACCOUNTS = 'eth_requestAccounts';

/** Minimal provider surface (MetaMask / Boing Express). */
export type Eip1193Requester = {
  request: (args: { method: string; params?: unknown }) => Promise<unknown>;
};

function asRequester(v: unknown): Eip1193Requester | undefined {
  if (v != null && typeof v === 'object' && typeof (v as Eip1193Requester).request === 'function') {
    return v as Eip1193Requester;
  }
  return undefined;
}

/**
 * Prefer **`window.boing`**, then **`window.ethereum`**, when both expose **`.request`**.
 */
export function getInjectedEip1193Provider(globalObj: typeof globalThis = globalThis): Eip1193Requester | undefined {
  const g = globalObj as Record<string, unknown>;
  return asRequester(g.boing) ?? asRequester(g.ethereum);
}

/**
 * True if the wallet speaks Boing JSON-RPC aliases (**`boing_chainId`**), without sending a transaction.
 */
export async function providerSupportsBoingNativeRpc(provider: Eip1193Requester): Promise<boolean> {
  try {
    const r = await provider.request({ method: BOING_CHAIN, params: [] });
    return typeof r === 'string' && r.startsWith('0x');
  } catch {
    return false;
  }
}

/** Call **`boing_sendTransaction`**; returns transaction hash string from the wallet. */
export async function boingSendTransaction(
  provider: Eip1193Requester,
  tx: Record<string, unknown>,
): Promise<string> {
  const out = await provider.request({ method: BOING_SEND, params: [tx] });
  if (typeof out !== 'string') {
    throw new Error('boing_sendTransaction: expected string tx hash from wallet');
  }
  return out;
}

/** **`boing_requestAccounts`** first, then **`eth_requestAccounts`**. */
export async function requestAccounts(provider: Eip1193Requester): Promise<string[]> {
  try {
    const a = await provider.request({ method: BOING_ACCOUNTS, params: [] });
    if (Array.isArray(a) && a.every((x) => typeof x === 'string')) return a as string[];
  } catch {
    /* fall through */
  }
  const a = await provider.request({ method: ETH_ACCOUNTS, params: [] });
  if (!Array.isArray(a) || !a.every((x) => typeof x === 'string')) {
    throw new Error('requestAccounts: wallet did not return string[]');
  }
  return a as string[];
}

/** Read **`boing_chainId`** or **`eth_chainId`** (hex string). */
export async function readChainIdHex(provider: Eip1193Requester): Promise<string> {
  try {
    const id = await provider.request({ method: BOING_CHAIN, params: [] });
    if (typeof id === 'string' && id.startsWith('0x')) return id.toLowerCase();
  } catch {
    /* fall through */
  }
  const id = await provider.request({ method: ETH_CHAIN, params: [] });
  if (typeof id !== 'string' || !id.startsWith('0x')) {
    throw new Error('readChainIdHex: wallet did not return 0x-prefixed chain id');
  }
  return id.toLowerCase();
}
