/**
 * High-level transaction construction + Ed25519 signing (browser/Node).
 * Produces hex `SignedTransaction` for `boing_submitTransaction`.
 */
import * as ed25519 from '@noble/ed25519';
import { encodeSignedTransaction, signableTransactionHash, } from './bincode.js';
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
