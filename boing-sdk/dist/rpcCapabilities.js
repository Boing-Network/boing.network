/**
 * Optional `boing_*` JSON-RPC methods: probe availability without assuming a full node build.
 * Uses JSON-RPC **-32601** (method not found) to detect missing handlers — same as integration fallbacks.
 */
import { BoingRpcError, isBoingRpcMethodNotFound } from './errors.js';
function mapProbeError(e) {
    if (isBoingRpcMethodNotFound(e)) {
        const msg = e instanceof Error ? e.message : String(e);
        return { available: false, code: -32601, message: msg };
    }
    if (e instanceof BoingRpcError) {
        return { available: false, code: e.code, message: e.message };
    }
    return { available: false, message: e instanceof Error ? e.message : String(e) };
}
const ZERO_TX_HEX = `0x${'00'.repeat(32)}`;
const CAP_KEYS = [
    'boing_chainHeight',
    'boing_getSyncState',
    'boing_getBlockByHeight',
    'boing_getLogs',
    'boing_getTransactionReceipt',
    'boing_getNetworkInfo',
];
function unwrapMethods(bundleOrMethods) {
    return 'methods' in bundleOrMethods ? bundleOrMethods.methods : bundleOrMethods;
}
/**
 * Call discovery plus a small set of read-only RPCs with minimal parameters.
 * A method is **available** if it returns (including JSON-RPC `result: null` for unknown tx/block).
 */
export async function probeBoingRpcCapabilities(client) {
    const methods = {};
    let clientVersion = null;
    let supportedMethods = null;
    await Promise.all([
        (async () => {
            try {
                clientVersion = await client.clientVersion();
            }
            catch (e) {
                if (!isBoingRpcMethodNotFound(e))
                    throw e;
            }
        })(),
        (async () => {
            try {
                supportedMethods = await client.rpcSupportedMethods();
            }
            catch (e) {
                if (!isBoingRpcMethodNotFound(e))
                    throw e;
            }
        })(),
        (async () => {
            try {
                await client.chainHeight();
                methods.boing_chainHeight = { available: true };
            }
            catch (e) {
                methods.boing_chainHeight = mapProbeError(e);
            }
        })(),
        (async () => {
            try {
                await client.getSyncState();
                methods.boing_getSyncState = { available: true };
            }
            catch (e) {
                methods.boing_getSyncState = mapProbeError(e);
            }
        })(),
        (async () => {
            try {
                await client.getBlockByHeight(0, false);
                methods.boing_getBlockByHeight = { available: true };
            }
            catch (e) {
                methods.boing_getBlockByHeight = mapProbeError(e);
            }
        })(),
        (async () => {
            try {
                await client.getLogs({ fromBlock: 0, toBlock: 0 });
                methods.boing_getLogs = { available: true };
            }
            catch (e) {
                methods.boing_getLogs = mapProbeError(e);
            }
        })(),
        (async () => {
            try {
                await client.getTransactionReceipt(ZERO_TX_HEX);
                methods.boing_getTransactionReceipt = { available: true };
            }
            catch (e) {
                methods.boing_getTransactionReceipt = mapProbeError(e);
            }
        })(),
        (async () => {
            try {
                await client.getNetworkInfo();
                methods.boing_getNetworkInfo = { available: true };
            }
            catch (e) {
                methods.boing_getNetworkInfo = mapProbeError(e);
            }
        })(),
    ]);
    return { clientVersion, supportedMethods, methods };
}
/** Count of core probed methods with `available: true`. */
export function countAvailableBoingRpcMethods(bundleOrMethods) {
    const m = unwrapMethods(bundleOrMethods);
    return CAP_KEYS.filter((k) => m[k].available).length;
}
/**
 * Human-readable diagnosis when **`probeBoingRpcCapabilities`** shows gaps.
 * **`undefined`** when all probed methods are available, or when there is nothing actionable to say.
 */
export function explainBoingRpcProbeGaps(bundleOrMethods) {
    const methods = unwrapMethods(bundleOrMethods);
    const supportedListing = 'supportedMethods' in bundleOrMethods ? bundleOrMethods.supportedMethods : undefined;
    const reportedVersion = 'clientVersion' in bundleOrMethods ? bundleOrMethods.clientVersion : undefined;
    if (!methods.boing_chainHeight.available) {
        return ('boing_chainHeight failed — this URL may not be a Boing JSON-RPC server (POST /, JSON-RPC 2.0), or the node is unreachable.');
    }
    const methodNotFound = CAP_KEYS.filter((k) => !methods[k].available && methods[k].code === -32601);
    if (methodNotFound.length > 0) {
        let s = `The server returned -32601 (method not found) for: ${methodNotFound.join(', ')}. ` +
            'The current **boing-node** in the boing.network repo registers these methods on the same HTTP POST / handler as boing_chainHeight. ' +
            'Typical causes: (1) an **older boing-node binary** — rebuild with `cargo build -p boing-node --release` and restart (see docs/RUNBOOK.md); ' +
            '(2) a **reverse proxy** that only forwards a subset of JSON-RPC method names. ' +
            'Full list: docs/RPC-API-SPEC.md.';
        if (supportedListing != null) {
            const sm = new Set(supportedListing);
            const ghost = methodNotFound.filter((k) => sm.has(k));
            if (ghost.length > 0) {
                s += ` Contradiction: boing_rpcSupportedMethods lists ${ghost.join(', ')} but calls failed — likely a filtering proxy.`;
            }
        }
        if (reportedVersion != null && reportedVersion !== '') {
            s += ` Reported client: ${reportedVersion}.`;
        }
        else if ('clientVersion' in bundleOrMethods &&
            bundleOrMethods.clientVersion === null) {
            s +=
                ' **`boing_clientVersion` is also missing (-32601)** — this process does not match a current `boing-node` from this repository (that method is registered alongside `boing_chainHeight`). ';
        }
        s +=
            ' Confirm what is listening: from the repo root run **`npm run rpc-endpoint-check`** (no SDK build). On Windows: **`netstat -ano | findstr :8545`** then **`tasklist /FI "PID eq <pid>" /V`**; start the node with **`cargo run -p boing-node -- --validator --rpc-port 8545`** or **`target/release/boing-node`** after **`cargo build -p boing-node --release`.**';
        return s;
    }
    const otherFailed = CAP_KEYS.filter((k) => !methods[k].available && methods[k].code !== -32601);
    if (otherFailed.length > 0) {
        return `Some probed methods failed with errors other than -32601: ${otherFailed.join(', ')}. Inspect methods.*.message in this output.`;
    }
    return undefined;
}
