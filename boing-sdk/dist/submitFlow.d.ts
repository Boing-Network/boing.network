/**
 * Simulate → refine access list → submit flows (roadmap P4).
 */
import type { BoingClient } from './client.js';
import type { SimulateResult } from './types.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
/** Thrown when simulation fails and access-list widening cannot proceed (or retries exhausted). */
export declare class SimulationFailedError extends Error {
    readonly simulation: SimulateResult;
    readonly attempt: number;
    constructor(message: string, simulation: SimulateResult, attempt: number);
}
export interface SubmitTransferWithSimulationOptions {
    client: BoingClient;
    secretKey32: Ed25519SecretKey32;
    senderHex: string;
    toHex: string;
    amount: bigint;
    /** Defaults to sender+to on both sides (matches parallel scheduling hint for transfers). */
    accessList?: {
        read: string[];
        write: string[];
    };
    /** Default 6. */
    maxSimulationRetries?: number;
}
export interface SubmitContractCallWithSimulationOptions {
    client: BoingClient;
    secretKey32: Ed25519SecretKey32;
    senderHex: string;
    contractHex: string;
    calldata: Uint8Array;
    /** Defaults to sender+contract on both sides. */
    accessList?: {
        read: string[];
        write: string[];
    };
    maxSimulationRetries?: number;
}
export interface SubmitFlowResult {
    tx_hash: string;
    lastSimulation: SimulateResult;
    attempts: number;
}
/**
 * Sign a transfer, run `boing_simulateTransaction`, widen access lists when the node reports
 * `access_list_covers_suggestion: false`, then `boing_submitTransaction` after a successful sim.
 */
export declare function submitTransferWithSimulationRetry(opts: SubmitTransferWithSimulationOptions): Promise<SubmitFlowResult>;
/** Same pattern as transfer for `ContractCall` payloads. */
export declare function submitContractCallWithSimulationRetry(opts: SubmitContractCallWithSimulationOptions): Promise<SubmitFlowResult>;
/**
 * Run `boing_qaCheck` (reject stops here), then sign → simulate → submit a purpose-bearing deploy.
 * Mempool may still return `-32050` / `-32051` / pool caps — catch `BoingRpcError` on submit.
 */
export declare function submitDeployWithPurposeFlow(opts: {
    client: BoingClient;
    secretKey32: Ed25519SecretKey32;
    senderHex: string;
    bytecode: Uint8Array;
    purposeCategory: string;
    descriptionHash?: Uint8Array | null;
    create2Salt?: Uint8Array | null;
    accessList?: {
        read: string[];
        write: string[];
    };
    maxSimulationRetries?: number;
}): Promise<SubmitFlowResult>;
//# sourceMappingURL=submitFlow.d.ts.map