/**
 * Copy, preflight, and one-shot tx helpers for native Boing DEX integration.
 * Reinforces that Boing is **not** a drop-in Uniswap/EVM router and keeps flows **Boing-RPC-only**.
 */
import type { BoingClient } from './client.js';
import type { NativeDexIntegrationDefaults } from './dexIntegration.js';
import { buildNativeConstantProductContractCallTx, type NativePoolAccessListOptions } from './nativeAmmPool.js';
import { type BoingRpcPreflightError } from './preflightGate.js';
/** Short line for tooltips / footers. */
export declare const BOING_NATIVE_DEX_NOT_EVM_TAGLINE = "Boing native DEX uses 32-byte account ids, explicit access lists, and VM-specific calldata\u2014not a paste-any-Uniswap-address router.";
/** Bullet list for onboarding modals or docs snippets. */
export declare const BOING_NATIVE_DEX_NOT_EVM_BULLETS: readonly ["Accounts are 32-byte Boing AccountIds (64 hex chars), not 20-byte Ethereum contract addresses.", "Each `contract_call` needs calldata per NATIVE-AMM / NATIVE-DEX specs plus explicit `access_list` read/write sets.", "Pool, factory, multihop router, ledger forwarders, and LP vault/share ids resolve via RPC `end_user` hints, `fetchNativeDexIntegrationDefaults`, env overrides, or CREATE2 prediction—there is no universal router ABI."];
export declare function formatBoingNativeDexNotEvmDisclaimer(): string;
/**
 * Surfaces missing operator hints so UIs can link to ops runbooks instead of failing silently.
 */
export declare function describeNativeDexDefaultGaps(defaults: NativeDexIntegrationDefaults): readonly string[];
/** RPC methods a typical native DEX dApp (read + simulate + logs) expects from **boing-node**. */
export declare const BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS: readonly ["boing_getNetworkInfo", "boing_simulateTransaction", "boing_getLogs", "boing_getContractStorage"];
/**
 * {@link assertBoingRpcEnvironment} with {@link BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS}.
 * Throws {@link BoingRpcPreflightError} when the endpoint is missing methods or failing discovery.
 */
export declare function assertBoingNativeDexToolkitRpc(client: BoingClient): Promise<import("./rpcDoctor.js").BoingRpcDoctorResult>;
/** User-facing text when {@link assertBoingNativeDexToolkitRpc} fails (QA / pruned / old node). */
export declare function formatNativeDexToolkitPreflightForUi(err: BoingRpcPreflightError): string;
/**
 * One-shot **`contract_call`** object for **`boing_sendTransaction`** / Boing Express: native CP **`swap`** calldata + pool access list.
 */
export declare function buildNativeCpPoolSwapExpressTx(input: {
    senderHex32: string;
    poolHex32: string;
    direction: bigint;
    amountIn: bigint;
    minOut: bigint;
    poolAccessListOptions?: NativePoolAccessListOptions;
}): ReturnType<typeof buildNativeConstantProductContractCallTx>;
//# sourceMappingURL=nativeDexSeamless.d.ts.map