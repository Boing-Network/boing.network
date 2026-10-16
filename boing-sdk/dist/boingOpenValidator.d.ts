/**
 * Open (permissionless) validator registration helpers — `TransactionPayload::ValidatorRegister /
 * ValidatorDeregister / ValidatorUnjail` and `boing_getValidators`.
 *
 * The **owner** account signs the transaction and pays the bond (default 100 BOING). The **node**
 * (consensus) key signs a proof `BLAKE3("BOING_OPEN_VALIDATOR_REGISTER_V1" || owner || nodeId)`, which
 * binds that node key to that owner. Owner and node may be the same key.
 *
 * Only accepted once the network has activated the `open` validator-set mode; check
 * `getBoingValidators(client).mode === 'open'` and `open.activation_height`.
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
export declare const BOING_VALIDATOR_REGISTRATION_DOMAIN = "BOING_OPEN_VALIDATOR_REGISTER_V1";
/** 32-byte message the node key signs (matches Rust `validator_registration_message`). */
export declare function validatorRegistrationMessage(ownerHex: string, nodeIdHex: string): Uint8Array;
/** Node-key proof of possession for `ownerHex`. Returns the node id and the 64-byte signature. */
export declare function signValidatorRegistration(ownerHex: string, nodeSecretKey32: Ed25519SecretKey32): Promise<{
    nodeIdHex: string;
    nodeSignature: Uint8Array;
}>;
/** Verify a registration proof (same check the node runs). */
export declare function verifyValidatorRegistration(ownerHex: string, nodeIdHex: string, nodeSignature: Uint8Array): Promise<boolean>;
export declare function buildValidatorRegisterTransaction(input: {
    nonce: bigint;
    ownerHex: string;
    nodeIdHex: string;
    nodeSignature: Uint8Array;
}): TransactionInput;
export declare function buildValidatorDeregisterTransaction(input: {
    nonce: bigint;
    ownerHex: string;
    nodeIdHex: string;
}): TransactionInput;
export declare function buildValidatorUnjailTransaction(input: {
    nonce: bigint;
    ownerHex: string;
    nodeIdHex: string;
}): TransactionInput;
export type BoingValidatorStatus = 'active' | 'pending' | 'jailed' | 'exiting';
/** One row of `boing_getValidators`. */
export interface BoingValidatorInfo {
    id: string;
    kind: 'anchor' | 'static' | 'open';
    status: BoingValidatorStatus;
    in_set: boolean;
    owner: string | null;
    bond: string;
    registered_at: number | null;
    jailed_until: number | null;
    jail_count: number;
    /** Liveness at the most recent jailing (current nodes; null if not recorded). The live window resets on jailing. */
    last_jail?: {
        jailed_at: number;
        scheduled_proposals: number;
        missed_proposals: number;
        uptime_pct: number;
    } | null;
    scheduled_proposals: number;
    missed_proposals: number;
    uptime_pct: number;
}
export interface BoingOpenValidatorConfig {
    activation_height: number;
    activated_at: number | null;
    bond: string;
    deregister_delay: number;
    epoch_len: number;
    next_epoch_height: number | null;
    max_open_validators: number;
    liveness_window: number;
    miss_threshold_bps: number;
    min_liveness_samples: number;
    jail_cooldown: number;
    anchors: string[];
}
export interface BoingValidatorsReport {
    mode: 'static' | 'stake' | 'open';
    height: number;
    quorum: number;
    validator_count: number;
    open: BoingOpenValidatorConfig | null;
    liveness_note?: string;
    validators: BoingValidatorInfo[];
}
/** `boing_getValidators` (nodes with open-mode support). */
export declare function getBoingValidators(client: BoingClient): Promise<BoingValidatorsReport>;
/** Find one validator by node id (case-insensitive). */
export declare function findBoingValidator(report: BoingValidatorsReport, nodeIdHex: string): BoingValidatorInfo | undefined;
/** True when the node accepts registrations at its next block. */
export declare function boingOpenRegistrationOpen(report: BoingValidatorsReport): boolean;
/**
 * Register `nodeSecretKey32`'s node as an open validator, paying the bond from `ownerSecretKey32`.
 * Pass the same key twice to self-own. Returns the submit result and the node id.
 */
export declare function registerBoingValidator(client: BoingClient, ownerSecretKey32: Ed25519SecretKey32, nodeSecretKey32: Ed25519SecretKey32): Promise<{
    nodeIdHex: string;
    result: import("./types.js").SubmitTransactionResult;
}>;
export declare function deregisterBoingValidator(client: BoingClient, ownerSecretKey32: Ed25519SecretKey32, nodeIdHex: string): Promise<import("./types.js").SubmitTransactionResult>;
export declare function unjailBoingValidator(client: BoingClient, ownerSecretKey32: Ed25519SecretKey32, nodeIdHex: string): Promise<import("./types.js").SubmitTransactionResult>;
/** Hex helper for display. */
export declare function nodeSignatureHex(sig: Uint8Array): string;
//# sourceMappingURL=boingOpenValidator.d.ts.map