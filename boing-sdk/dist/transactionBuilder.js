/**
 * High-level transaction construction + Ed25519 signing (browser/Node).
 * Produces hex `SignedTransaction` for `boing_submitTransaction`.
 */
import * as ed25519 from '@noble/ed25519';
import { MAX_BATCH_CALLS, validateBatchCalls, encodeSignedTransaction, signableTransactionHash, } from './bincode.js';
import { bytesToHex, hexToAccountId, validateHex32 } from './hex.js';
/** Derive Boing `AccountId` hex (0x + 64 hex) from a 32-byte Ed25519 secret seed. */
export async function senderHexFromSecretKey(secretKey32) {
    if (secretKey32.length !== 32)
        throw new Error('secret key must be 32 bytes');
    const pub = await ed25519.getPublicKeyAsync(secretKey32);
    return validateHex32(bytesToHex(pub));
}
/** Next tx nonce from chain (`boing_getAccount`). */
export async function fetchNextNonce(client, senderHex) {
    const acc = await client.getAccount(validateHex32(senderHex));
    return BigInt(acc.nonce);
}
function accountVec(hexList) {
    return hexList.map((h) => hexToAccountId(validateHex32(h)));
}
export function buildTransferTransaction(input) {
    const read = input.accessList?.read ?? [];
    const write = input.accessList?.write ?? [];
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.senderHex)),
        payload: {
            kind: 'transfer',
            to: hexToAccountId(validateHex32(input.toHex)),
            amount: input.amount,
        },
        accessList: { read: accountVec(read), write: accountVec(write) },
    };
}
export function buildContractCallTransaction(input) {
    const read = input.accessList?.read ?? [];
    const write = input.accessList?.write ?? [];
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.senderHex)),
        payload: {
            kind: 'contractCall',
            contract: hexToAccountId(validateHex32(input.contractHex)),
            calldata: input.calldata,
        },
        accessList: { read: accountVec(read), write: accountVec(write) },
    };
}
/**
 * Native atomic batch (`TransactionPayload::Batch`): BOING transfers and/or contract calls
 * (e.g. token `transfer`, NFT transfer calldata) under one signature and one nonce.
 * Requires nodes running the batch-capable release.
 */
export function buildBatchTransaction(input) {
    const sender = validateHex32(input.senderHex);
    const calls = input.calls.map((c) => c.kind === 'transfer'
        ? { kind: 'transfer', to: hexToAccountId(validateHex32(c.toHex)), amount: c.amount }
        : {
            kind: 'contractCall',
            contract: hexToAccountId(validateHex32(c.contractHex)),
            calldata: c.calldata,
        });
    validateBatchCalls(calls);
    let read;
    let write;
    if (input.accessList) {
        read = input.accessList.read;
        write = input.accessList.write;
    }
    else {
        const ids = [sender.toLowerCase()];
        for (const c of input.calls) {
            const id = validateHex32(c.kind === 'transfer' ? c.toHex : c.contractHex).toLowerCase();
            if (!ids.includes(id))
                ids.push(id);
        }
        read = ids;
        write = ids;
    }
    return {
        nonce: input.nonce,
        sender: hexToAccountId(sender),
        payload: { kind: 'batch', calls },
        accessList: { read: accountVec(read), write: accountVec(write) },
    };
}
/**
 * `true` when `boing_health.rpc_surface` advertises native batch support. Older nodes omit the
 * flag (→ `false`), so callers should fall back to sequential transfers.
 */
export function nodeSupportsNativeBatch(health) {
    return health?.rpc_surface?.tx_payload_batch === true;
}
/** Batch step cap advertised by the node, else the protocol default (16). */
export function nodeBatchMaxCalls(health) {
    const n = health?.rpc_surface?.tx_batch_max_calls;
    return typeof n === 'number' && n > 0 ? Math.min(n, MAX_BATCH_CALLS) : MAX_BATCH_CALLS;
}
export function buildDeployWithPurposeTransaction(input) {
    const read = input.accessList?.read ?? [];
    const write = input.accessList?.write ?? [];
    return {
        nonce: input.nonce,
        sender: hexToAccountId(validateHex32(input.senderHex)),
        payload: {
            kind: 'contractDeployWithPurpose',
            bytecode: input.bytecode,
            purposeCategory: input.purposeCategory,
            descriptionHash: input.descriptionHash ?? null,
            create2Salt: input.create2Salt ?? null,
        },
        accessList: { read: accountVec(read), write: accountVec(write) },
    };
}
/**
 * Sign the Boing signable hash with Ed25519 and return bincode `SignedTransaction` as 0x-hex.
 */
function bytesEqual(a, b) {
    if (a.length !== b.length)
        return false;
    for (let i = 0; i < a.length; i++)
        if (a[i] !== b[i])
            return false;
    return true;
}
export async function signTransactionInput(tx, secretKey32) {
    if (secretKey32.length !== 32)
        throw new Error('secret key must be 32 bytes');
    const pub = await ed25519.getPublicKeyAsync(secretKey32);
    if (!bytesEqual(pub, tx.sender)) {
        throw new Error('tx.sender must equal the Ed25519 public key derived from the secret key');
    }
    const hash = signableTransactionHash(tx);
    const sig = await ed25519.signAsync(hash, secretKey32);
    const raw = encodeSignedTransaction(tx, sig);
    return bytesToHex(raw);
}
/**
 * Sign with a custom async signer (e.g. hardware wallet or IPC to Boing Express).
 * Caller must ensure the signer corresponds to `tx.sender`; no verification here.
 */
export async function signTransactionInputWithSigner(tx, signHash) {
    const hash = signableTransactionHash(tx);
    const sig = await signHash(hash);
    if (sig.length !== 64)
        throw new Error('Ed25519 signature must be 64 bytes');
    return bytesToHex(encodeSignedTransaction(tx, sig));
}
