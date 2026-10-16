/**
 * Open (permissionless) validator registration helpers — `TransactionPayload::ValidatorRegister /
 * ValidatorDeregister / ValidatorUnjail` and `boing_getValidators`.
 *
 * The **owner** account signs the transaction and pays the bond (default 100 BOING). The **node**
 * (consensus) key signs a proof `BLAKE3("BOING_OPEN_VALIDATOR_REGISTER_V1" || owner || nodeId)`, which
 * binds that node key to that owner. Owner and node may be the same key. Nodes that advertise a chain id
 * (`boing_health.tx_signing`) use the chain-bound v2 proof
 * `BLAKE3("BOING_OPEN_VALIDATOR_REGISTER_V2" || chain_id_le8 || owner || nodeId)`, required from
 * `registration_proof_v2_from_height` (security review #14).
 *
 * Only accepted once the network has activated the `open` validator-set mode; check
 * `getBoingValidators(client).mode === 'open'` and `open.activation_height`.
 */
import * as ed25519 from '@noble/ed25519';
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, hexToAccountId, validateHex32 } from './hex.js';
import { fetchNextNonce, fetchSigningChainId, senderHexFromSecretKey, signTransactionInputForClient, } from './transactionBuilder.js';
export const BOING_VALIDATOR_REGISTRATION_DOMAIN = 'BOING_OPEN_VALIDATOR_REGISTER_V1';
/** 32-byte message the node key signs (matches Rust `validator_registration_message`). */
export function validatorRegistrationMessage(ownerHex, nodeIdHex) {
    const dom = new TextEncoder().encode(BOING_VALIDATOR_REGISTRATION_DOMAIN);
    const owner = hexToAccountId(validateHex32(ownerHex));
    const node = hexToAccountId(validateHex32(nodeIdHex));
    const pre = new Uint8Array(dom.length + 64);
    pre.set(dom, 0);
    pre.set(owner, dom.length);
    pre.set(node, dom.length + 32);
    return blake3(pre);
}
export const BOING_VALIDATOR_REGISTRATION_DOMAIN_V2 = 'BOING_OPEN_VALIDATOR_REGISTER_V2';
/** Chain-bound (v2) message (matches Rust `validator_registration_message_v2`). */
export function validatorRegistrationMessageV2(ownerHex, nodeIdHex, chainId) {
    const dom = new TextEncoder().encode(BOING_VALIDATOR_REGISTRATION_DOMAIN_V2);
    const owner = hexToAccountId(validateHex32(ownerHex));
    const node = hexToAccountId(validateHex32(nodeIdHex));
    const pre = new Uint8Array(dom.length + 8 + 64);
    pre.set(dom, 0);
    new DataView(pre.buffer).setBigUint64(dom.length, BigInt(chainId), true);
    pre.set(owner, dom.length + 8);
    pre.set(node, dom.length + 40);
    return blake3(pre);
}
function registrationMessage(ownerHex, nodeIdHex, chainId) {
    return chainId == null
        ? validatorRegistrationMessage(ownerHex, nodeIdHex)
        : validatorRegistrationMessageV2(ownerHex, nodeIdHex, chainId);
}
/**
 * Node-key proof of possession for `ownerHex`. Returns the node id and the 64-byte signature.
 * With `chainId` the chain-bound v2 proof is produced (what nodes with a chain id require).
 */
export async function signValidatorRegistration(ownerHex, nodeSecretKey32, opts = {}) {
    const nodeIdHex = await senderHexFromSecretKey(nodeSecretKey32);
    const sig = await ed25519.signAsync(registrationMessage(ownerHex, nodeIdHex, opts.chainId), nodeSecretKey32);
    return { nodeIdHex, nodeSignature: sig };
}
/** Verify a registration proof (v2 for `chainId` when given, else v1). */
export async function verifyValidatorRegistration(ownerHex, nodeIdHex, nodeSignature, chainId) {
    if (nodeSignature.length !== 64)
        return false;
    try {
        return await ed25519.verifyAsync(nodeSignature, registrationMessage(ownerHex, nodeIdHex, chainId), hexToAccountId(validateHex32(nodeIdHex)));
    }
    catch {
        return false;
    }
}
function selfAccessList(ownerHex) {
    const id = hexToAccountId(validateHex32(ownerHex));
    return { read: [id], write: [id] };
}
export function buildValidatorRegisterTransaction(input) {
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.ownerHex)),
        payload: {
            kind: 'validatorRegister',
            nodeId: hexToAccountId(validateHex32(input.nodeIdHex)),
            nodeSignature: input.nodeSignature,
        },
        accessList: selfAccessList(input.ownerHex),
    };
}
export function buildValidatorDeregisterTransaction(input) {
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.ownerHex)),
        payload: { kind: 'validatorDeregister', nodeId: hexToAccountId(validateHex32(input.nodeIdHex)) },
        accessList: selfAccessList(input.ownerHex),
    };
}
export function buildValidatorUnjailTransaction(input) {
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.ownerHex)),
        payload: { kind: 'validatorUnjail', nodeId: hexToAccountId(validateHex32(input.nodeIdHex)) },
        accessList: selfAccessList(input.ownerHex),
    };
}
/** `boing_getValidators` (nodes with open-mode support). */
export async function getBoingValidators(client) {
    return client.getValidators();
}
/** Find one validator by node id (case-insensitive). */
export function findBoingValidator(report, nodeIdHex) {
    const id = validateHex32(nodeIdHex).toLowerCase();
    return report.validators.find((v) => v.id.toLowerCase() === id);
}
/** True when the node accepts registrations at its next block. */
export function boingOpenRegistrationOpen(report) {
    return report.mode === 'open' && !!report.open && report.height + 1 >= report.open.activation_height;
}
async function signAndSubmit(client, tx, secret) {
    const hex = await signTransactionInputForClient(client, tx, secret);
    return client.submitTransaction(hex);
}
/**
 * Register `nodeSecretKey32`'s node as an open validator, paying the bond from `ownerSecretKey32`.
 * Pass the same key twice to self-own. Returns the submit result and the node id.
 */
export async function registerBoingValidator(client, ownerSecretKey32, nodeSecretKey32) {
    const ownerHex = await senderHexFromSecretKey(ownerSecretKey32);
    const chainId = await fetchSigningChainId(client);
    const { nodeIdHex, nodeSignature } = await signValidatorRegistration(ownerHex, nodeSecretKey32, { chainId });
    const nonce = await fetchNextNonce(client, ownerHex);
    const tx = buildValidatorRegisterTransaction({ nonce, ownerHex, nodeIdHex, nodeSignature });
    return { nodeIdHex, result: await signAndSubmit(client, tx, ownerSecretKey32) };
}
export async function deregisterBoingValidator(client, ownerSecretKey32, nodeIdHex) {
    const ownerHex = await senderHexFromSecretKey(ownerSecretKey32);
    const nonce = await fetchNextNonce(client, ownerHex);
    return signAndSubmit(client, buildValidatorDeregisterTransaction({ nonce, ownerHex, nodeIdHex }), ownerSecretKey32);
}
export async function unjailBoingValidator(client, ownerSecretKey32, nodeIdHex) {
    const ownerHex = await senderHexFromSecretKey(ownerSecretKey32);
    const nonce = await fetchNextNonce(client, ownerHex);
    return signAndSubmit(client, buildValidatorUnjailTransaction({ nonce, ownerHex, nodeIdHex }), ownerSecretKey32);
}
/** Hex helper for display. */
export function nodeSignatureHex(sig) {
    return bytesToHex(sig);
}
