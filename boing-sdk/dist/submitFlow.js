/**
 * Simulate → refine access list → submit flows (roadmap P4).
 */
import { mergeAccessListWithSimulation } from './accessList.js';
import { BoingRpcError } from './errors.js';
import { bytesToHex } from './hex.js';
import { buildContractCallTransaction, buildDeployWithPurposeTransaction, buildTransferTransaction, fetchNextNonce, signTransactionInput, } from './transactionBuilder.js';
/** Thrown when simulation fails and access-list widening cannot proceed (or retries exhausted). */
export class SimulationFailedError extends Error {
    constructor(message, simulation, attempt) {
        super(message);
        this.simulation = simulation;
        this.attempt = attempt;
        this.name = 'SimulationFailedError';
    }
}
/**
 * Sign a transfer, run `boing_simulateTransaction`, widen access lists when the node reports
 * `access_list_covers_suggestion: false`, then `boing_submitTransaction` after a successful sim.
 */
export async function submitTransferWithSimulationRetry(opts) {
    const max = opts.maxSimulationRetries ?? 6;
    let read = opts.accessList?.read ?? [opts.senderHex, opts.toHex];
    let write = opts.accessList?.write ?? [opts.senderHex, opts.toHex];
    const nonce = await fetchNextNonce(opts.client, opts.senderHex);
    let lastSim = null;
    for (let attempt = 0; attempt < max; attempt++) {
        const tx = buildTransferTransaction({
            nonce,
            senderHex: opts.senderHex,
            toHex: opts.toHex,
            amount: opts.amount,
            accessList: { read, write },
        });
        const signedHex = await signTransactionInput(tx, opts.secretKey32);
        lastSim = await opts.client.simulateTransaction(signedHex);
        if (lastSim.success) {
            const sub = await opts.client.submitTransaction(signedHex);
            return { tx_hash: sub.tx_hash, lastSimulation: lastSim, attempts: attempt + 1 };
        }
        if (lastSim.access_list_covers_suggestion === false && lastSim.suggested_access_list) {
            const m = mergeAccessListWithSimulation(read, write, lastSim);
            read = m.read;
            write = m.write;
            continue;
        }
        throw new SimulationFailedError(lastSim.error ?? 'Simulation failed', lastSim, attempt);
    }
    throw new SimulationFailedError(`Exceeded maxSimulationRetries (${max})`, lastSim, max - 1);
}
/** Same pattern as transfer for `ContractCall` payloads. */
export async function submitContractCallWithSimulationRetry(opts) {
    const max = opts.maxSimulationRetries ?? 6;
    let read = opts.accessList?.read ?? [opts.senderHex, opts.contractHex];
    let write = opts.accessList?.write ?? [opts.senderHex, opts.contractHex];
    const nonce = await fetchNextNonce(opts.client, opts.senderHex);
    let lastSim = null;
    for (let attempt = 0; attempt < max; attempt++) {
        const tx = buildContractCallTransaction({
            nonce,
            senderHex: opts.senderHex,
            contractHex: opts.contractHex,
            calldata: opts.calldata,
            accessList: { read, write },
        });
        const signedHex = await signTransactionInput(tx, opts.secretKey32);
        lastSim = await opts.client.simulateTransaction(signedHex);
        if (lastSim.success) {
            const sub = await opts.client.submitTransaction(signedHex);
            return { tx_hash: sub.tx_hash, lastSimulation: lastSim, attempts: attempt + 1 };
        }
        if (lastSim.access_list_covers_suggestion === false && lastSim.suggested_access_list) {
            const m = mergeAccessListWithSimulation(read, write, lastSim);
            read = m.read;
            write = m.write;
            continue;
        }
        throw new SimulationFailedError(lastSim.error ?? 'Simulation failed', lastSim, attempt);
    }
    throw new SimulationFailedError(`Exceeded maxSimulationRetries (${max})`, lastSim, max - 1);
}
/**
 * Run `boing_qaCheck` (reject stops here), then sign → simulate → submit a purpose-bearing deploy.
 * Mempool may still return `-32050` / `-32051` / pool caps — catch `BoingRpcError` on submit.
 */
export async function submitDeployWithPurposeFlow(opts) {
    const qa = await opts.client.qaCheck(bytesToHex(opts.bytecode), opts.purposeCategory, opts.descriptionHash ? bytesToHex(opts.descriptionHash) : undefined);
    if (qa.result === 'reject') {
        throw new BoingRpcError(-32050, qa.message ?? 'QA preflight rejected', { rule_id: qa.rule_id, message: qa.message }, 'boing_qaCheck');
    }
    const max = opts.maxSimulationRetries ?? 6;
    let read = opts.accessList?.read ?? [opts.senderHex];
    let write = opts.accessList?.write ?? [opts.senderHex];
    const nonce = await fetchNextNonce(opts.client, opts.senderHex);
    let lastSim = null;
    for (let attempt = 0; attempt < max; attempt++) {
        const tx = buildDeployWithPurposeTransaction({
            nonce,
            senderHex: opts.senderHex,
            bytecode: opts.bytecode,
            purposeCategory: opts.purposeCategory,
            descriptionHash: opts.descriptionHash ?? null,
            create2Salt: opts.create2Salt ?? null,
            accessList: { read, write },
        });
        const signedHex = await signTransactionInput(tx, opts.secretKey32);
        lastSim = await opts.client.simulateTransaction(signedHex);
        if (lastSim.success) {
            const sub = await opts.client.submitTransaction(signedHex);
            return { tx_hash: sub.tx_hash, lastSimulation: lastSim, attempts: attempt + 1 };
        }
        if (lastSim.access_list_covers_suggestion === false && lastSim.suggested_access_list) {
            const m = mergeAccessListWithSimulation(read, write, lastSim);
            read = m.read;
            write = m.write;
            continue;
        }
        throw new SimulationFailedError(lastSim.error ?? 'Simulation failed', lastSim, attempt);
    }
    throw new SimulationFailedError(`Exceeded maxSimulationRetries (${max})`, lastSim, max - 1);
}
