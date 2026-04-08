/**
 * Copy, preflight, and one-shot tx helpers for native Boing DEX integration.
 * Reinforces that Boing is **not** a drop-in Uniswap/EVM router and keeps flows **Boing-RPC-only**.
 */
import { bytesToHex } from './hex.js';
import { encodeNativeAmmSwapCalldata } from './nativeAmm.js';
import { buildNativeConstantProductContractCallTx, } from './nativeAmmPool.js';
import { assertBoingRpcEnvironment } from './preflightGate.js';
/** Short line for tooltips / footers. */
export const BOING_NATIVE_DEX_NOT_EVM_TAGLINE = 'Boing native DEX uses 32-byte account ids, explicit access lists, and VM-specific calldata—not a paste-any-Uniswap-address router.';
/** Bullet list for onboarding modals or docs snippets. */
export const BOING_NATIVE_DEX_NOT_EVM_BULLETS = [
    'Accounts are 32-byte Boing AccountIds (64 hex chars), not 20-byte Ethereum contract addresses.',
    'Each `contract_call` needs calldata per NATIVE-AMM / NATIVE-DEX specs plus explicit `access_list` read/write sets.',
    'Pool, factory, multihop router, ledger forwarders, and LP vault/share ids resolve via RPC `end_user` hints, `fetchNativeDexIntegrationDefaults`, env overrides, or CREATE2 prediction—there is no universal router ABI.',
];
export function formatBoingNativeDexNotEvmDisclaimer() {
    const bullets = BOING_NATIVE_DEX_NOT_EVM_BULLETS.map((b) => `• ${b}`).join('\n');
    return `${BOING_NATIVE_DEX_NOT_EVM_TAGLINE}\n\n${bullets}`;
}
/**
 * Surfaces missing operator hints so UIs can link to ops runbooks instead of failing silently.
 */
export function describeNativeDexDefaultGaps(defaults) {
    const out = [];
    if (defaults.nativeCpPoolAccountHex == null) {
        out.push('No resolved native constant-product pool id: set node env BOING_CANONICAL_NATIVE_CP_POOL, use chain 6913 embedded fallback, or pass a pool override in your app.');
    }
    if (defaults.nativeDexFactoryAccountHex == null) {
        out.push('No native DEX factory id: set BOING_CANONICAL_NATIVE_DEX_FACTORY on the node or pass a factory override so directory scans and pair resolution can run.');
    }
    return out;
}
/** RPC methods a typical native DEX dApp (read + simulate + logs) expects from **boing-node**. */
export const BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS = [
    'boing_getNetworkInfo',
    'boing_simulateTransaction',
    'boing_getLogs',
    'boing_getContractStorage',
];
/**
 * {@link assertBoingRpcEnvironment} with {@link BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS}.
 * Throws {@link BoingRpcPreflightError} when the endpoint is missing methods or failing discovery.
 */
export async function assertBoingNativeDexToolkitRpc(client) {
    return assertBoingRpcEnvironment(client, {
        requiredMethods: [...BOING_NATIVE_DEX_TOOLKIT_RPC_METHODS],
    });
}
/** User-facing text when {@link assertBoingNativeDexToolkitRpc} fails (QA / pruned / old node). */
export function formatNativeDexToolkitPreflightForUi(err) {
    const lines = [
        'This screen needs a Boing JSON-RPC endpoint that supports simulate, bounded logs, contract storage, and network info.',
        'Deploy and sensitive paths stay gated by protocol QA—use the SDK preflight helpers and show users a clear review step.',
        '',
        ...err.doctor.messages,
    ];
    return lines.join('\n');
}
/**
 * One-shot **`contract_call`** object for **`boing_sendTransaction`** / Boing Express: native CP **`swap`** calldata + pool access list.
 */
export function buildNativeCpPoolSwapExpressTx(input) {
    const calldata = bytesToHex(encodeNativeAmmSwapCalldata(input.direction, input.amountIn, input.minOut));
    return buildNativeConstantProductContractCallTx(input.senderHex32, input.poolHex32, calldata, input.poolAccessListOptions);
}
