/**
 * Thin **EIP-1193** helpers for **Boing Express** and compatible injected providers.
 * Reduces branching in dApps that call **`boing_sendTransaction`**, **`boing_requestAccounts`**, **`boing_chainId`**.
 */
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
//# sourceMappingURL=walletProvider.d.ts.map