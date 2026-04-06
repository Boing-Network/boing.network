/**
 * Thin **EIP-1193** helpers for **Boing Express** and compatible injected providers.
 * Reduces branching in dApps that call **`boing_sendTransaction`**, **`boing_requestAccounts`**, **`boing_chainId`**.
 */
/**
 * EIP-1193 methods Boing-native dApps typically rely on (Boing Express implements these).
 * Generic `eth_sendTransaction` alone is **not** enough for Boing **`contract_call`** (32-byte ids + access lists).
 */
export declare const BOING_WALLET_RPC_METHODS_NATIVE_DAPP: readonly ["boing_chainId", "boing_requestAccounts", "boing_sendTransaction"];
/** Explains why **`eth_sendTransaction`**-centric wallets are insufficient for native Boing **`contract_call`**. */
export declare function explainEthSendTransactionInsufficientForBoingNativeCall(): string;
/** Minimal provider surface (MetaMask / Boing Express). */
export type Eip1193Requester = {
    request: (args: {
        method: string;
        params?: unknown;
    }) => Promise<unknown>;
};
/**
 * Prefer **`window.boing`**, then **`window.ethereum`**, when both expose **`.request`**.
 */
export declare function getInjectedEip1193Provider(globalObj?: typeof globalThis): Eip1193Requester | undefined;
/**
 * True if the wallet speaks Boing JSON-RPC aliases (**`boing_chainId`**), without sending a transaction.
 */
export declare function providerSupportsBoingNativeRpc(provider: Eip1193Requester): Promise<boolean>;
/** Call **`boing_sendTransaction`**; returns transaction hash string from the wallet. */
export declare function boingSendTransaction(provider: Eip1193Requester, tx: Record<string, unknown>): Promise<string>;
/** **`boing_requestAccounts`** first, then **`eth_requestAccounts`**. */
export declare function requestAccounts(provider: Eip1193Requester): Promise<string[]>;
/** Read **`boing_chainId`** or **`eth_chainId`** (hex string). */
export declare function readChainIdHex(provider: Eip1193Requester): Promise<string>;
export type BoingInjectedWalletConnectResult = {
    accounts: string[];
    chainIdHex: string;
    supportsBoingNativeRpc: boolean;
};
/**
 * Single **`requestAccounts`** + **`readChainIdHex`** + **`providerSupportsBoingNativeRpc`** — typical “Connect wallet” success payload.
 */
export declare function connectInjectedBoingWallet(provider: Eip1193Requester): Promise<BoingInjectedWalletConnectResult>;
/**
 * Map common injected-wallet errors to short UI strings (MetaMask-style **`code`** when present).
 */
export declare function mapInjectedProviderErrorToUiMessage(err: unknown): string;
//# sourceMappingURL=walletProvider.d.ts.map