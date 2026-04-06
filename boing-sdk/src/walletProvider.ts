/**
 * Thin **EIP-1193** helpers for **Boing Express** and compatible injected providers.
 * Reduces branching in dApps that call **`boing_sendTransaction`**, **`boing_requestAccounts`**, **`boing_chainId`**.
 */

const BOING_SEND = 'boing_sendTransaction';
const BOING_ACCOUNTS = 'boing_requestAccounts';
const BOING_CHAIN = 'boing_chainId';
const ETH_CHAIN = 'eth_chainId';
const ETH_ACCOUNTS = 'eth_requestAccounts';
/**
 * EIP-1193 methods Boing-native dApps typically rely on (Boing Express implements these).
 * Generic `eth_sendTransaction` alone is **not** enough for Boing **`contract_call`** (32-byte ids + access lists).
 */
export const BOING_WALLET_RPC_METHODS_NATIVE_DAPP = [
  BOING_CHAIN,
  BOING_ACCOUNTS,
  BOING_SEND,
] as const;

/** Explains why **`eth_sendTransaction`**-centric wallets are insufficient for native Boing **`contract_call`**. */
export function explainEthSendTransactionInsufficientForBoingNativeCall(): string {
  return [
    'Boing `contract_call` transactions use 32-byte account ids, explicit access lists, and bincode signing—not the implicit 20-byte `to`/`data` shape most `eth_sendTransaction` wallets assume.',
    'Use Boing Express (or an injected provider that implements `boing_sendTransaction` / `boing_chainId`) or sign server-side with `boing-sdk` and `boing_submitTransaction`.',
    `Methods to look for: ${BOING_WALLET_RPC_METHODS_NATIVE_DAPP.join(', ')}.`,
  ].join('\n');
}

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

export type BoingInjectedWalletConnectResult = {
  accounts: string[];
  chainIdHex: string;
  supportsBoingNativeRpc: boolean;
};

/**
 * Single **`requestAccounts`** + **`readChainIdHex`** + **`providerSupportsBoingNativeRpc`** — typical “Connect wallet” success payload.
 */
export async function connectInjectedBoingWallet(
  provider: Eip1193Requester
): Promise<BoingInjectedWalletConnectResult> {
  const [accounts, chainIdHex, supportsBoingNativeRpc] = await Promise.all([
    requestAccounts(provider),
    readChainIdHex(provider),
    providerSupportsBoingNativeRpc(provider),
  ]);
  return { accounts, chainIdHex, supportsBoingNativeRpc };
}

/**
 * Map common injected-wallet errors to short UI strings (MetaMask-style **`code`** when present).
 */
export function mapInjectedProviderErrorToUiMessage(err: unknown): string {
  const o = err as { code?: number; message?: string; data?: unknown };
  const code = typeof o?.code === 'number' ? o.code : undefined;
  const msg = typeof o?.message === 'string' ? o.message : '';

  if (code === 4001 || /user rejected|denied|rejected/i.test(msg)) {
    return 'Request was cancelled in the wallet.';
  }
  if (code === -32603 || /internal error/i.test(msg)) {
    return 'The wallet reported an internal error. Try again or switch networks.';
  }
  if (/method not found|not supported|does not exist/i.test(msg)) {
    return 'This wallet may not support Boing RPC methods. Use Boing Express or sign with boing-sdk on the server.';
  }
  if (/network|chain/i.test(msg) && /wrong|invalid|mismatch/i.test(msg)) {
    return 'Wrong network selected in the wallet. Switch to the Boing chain and retry.';
  }
  if (msg.trim().length > 0) {
    return msg.length > 200 ? `${msg.slice(0, 197)}…` : msg;
  }
  return 'Wallet request failed. Try again or use a Boing-compatible wallet.';
}
