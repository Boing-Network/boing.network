/**
 * bincode 1.3 layout matching `crates/boing-primitives` (serde derive).
 * Used for `Transaction`, `SignedTransaction`, and signable hash preimage.
 * @see docs/BOING-SIGNED-TRANSACTION-ENCODING.md
 */
import { blake3 } from '@noble/hashes/blake3';
/** Payload variants — discriminant u32 LE must match Rust enum declaration order. */
export const PayloadVariant = {
    Transfer: 0,
    ContractCall: 1,
    ContractDeploy: 2,
    ContractDeployWithPurpose: 3,
    ContractDeployWithPurposeAndMetadata: 4,
    Bond: 5,
    Unbond: 6,
};
export function concatBytes(...parts) {
    const n = parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(n);
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
export function writeU32Le(n) {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setUint32(0, n >>> 0, true);
    return b;
}
export function writeU64Le(n) {
    const b = new Uint8Array(8);
    new DataView(b.buffer).setBigUint64(0, n, true);
    return b;
}
export function writeU128Le(n) {
    const b = new Uint8Array(16);
    let x = n;
    for (let i = 0; i < 16; i++) {
        b[i] = Number(x & 0xffn);
        x >>= 8n;
    }
    return b;
}
function encodeVecAccountIds(ids) {
    const parts = [writeU64Le(BigInt(ids.length))];
    for (const id of ids) {
        if (id.length !== 32)
            throw new Error('Each AccountId must be exactly 32 bytes');
        parts.push(id);
    }
    return concatBytes(...parts);
}
/** `AccessList` — two length-prefixed vectors of 32-byte accounts. */
export function encodeAccessList(read, write) {
    return concatBytes(encodeVecAccountIds(read), encodeVecAccountIds(write));
}
/** `Vec<u8>` — u64 LE length + raw bytes. */
export function encodeByteVec(data) {
    return concatBytes(writeU64Le(BigInt(data.length)), data);
}
/** UTF-8 string: u64 LE byte length + UTF-8 payload. */
export function encodeBincodeString(s) {
    const utf8 = new TextEncoder().encode(s);
    return concatBytes(writeU64Le(BigInt(utf8.length)), utf8);
}
/** `Option<[u8;32]>` — tag u8 + optional fixed 32 bytes. */
export function encodeOptionFixed32(salt) {
    if (salt == null)
        return new Uint8Array([0]);
    if (salt.length !== 32)
        throw new Error('create2_salt must be 32 bytes');
    return concatBytes(new Uint8Array([1]), salt);
}
/** `Option<Vec<u8>>` — tag u8 + optional vec. */
export function encodeOptionByteVec(opt) {
    if (opt == null)
        return new Uint8Array([0]);
    return concatBytes(new Uint8Array([1]), encodeByteVec(opt));
}
/** `Option<String>` — tag u8 + optional string. */
export function encodeOptionString(opt) {
    if (opt == null)
        return new Uint8Array([0]);
    return concatBytes(new Uint8Array([1]), encodeBincodeString(opt));
}
export function encodeTransactionPayload(payload) {
    switch (payload.kind) {
        case 'transfer':
            return concatBytes(writeU32Le(PayloadVariant.Transfer), payload.to, writeU128Le(payload.amount));
        case 'contractCall':
            return concatBytes(writeU32Le(PayloadVariant.ContractCall), payload.contract, encodeByteVec(payload.calldata));
        case 'contractDeploy':
            return concatBytes(writeU32Le(PayloadVariant.ContractDeploy), encodeByteVec(payload.bytecode), encodeOptionFixed32(payload.create2Salt ?? null));
        case 'contractDeployWithPurpose':
            return concatBytes(writeU32Le(PayloadVariant.ContractDeployWithPurpose), encodeByteVec(payload.bytecode), encodeBincodeString(payload.purposeCategory), encodeOptionByteVec(payload.descriptionHash ?? null), encodeOptionFixed32(payload.create2Salt ?? null));
        case 'contractDeployWithPurposeAndMetadata':
            return concatBytes(writeU32Le(PayloadVariant.ContractDeployWithPurposeAndMetadata), encodeByteVec(payload.bytecode), encodeBincodeString(payload.purposeCategory), encodeOptionByteVec(payload.descriptionHash ?? null), encodeOptionString(payload.assetName ?? null), encodeOptionString(payload.assetSymbol ?? null), encodeOptionFixed32(payload.create2Salt ?? null));
        case 'bond':
            return concatBytes(writeU32Le(PayloadVariant.Bond), writeU128Le(payload.amount));
        case 'unbond':
            return concatBytes(writeU32Le(PayloadVariant.Unbond), writeU128Le(payload.amount));
        default: {
            const _x = payload;
            return _x;
        }
    }
}
export function encodeTransaction(tx) {
    if (tx.sender.length !== 32)
        throw new Error('sender must be 32 bytes');
    return concatBytes(writeU64Le(tx.nonce), tx.sender, encodeTransactionPayload(tx.payload), encodeAccessList(tx.accessList.read, tx.accessList.write));
}
/** `Signature` — serde `serialize_bytes` → u64 LE length + 64 raw bytes. */
export function encodeSignature(signature64) {
    if (signature64.length !== 64)
        throw new Error('Ed25519 signature must be 64 bytes');
    return concatBytes(writeU64Le(64n), signature64);
}
export function encodeSignedTransaction(tx, signature64) {
    return concatBytes(encodeTransaction(tx), encodeSignature(signature64));
}
/**
 * BLAKE3 hash signed by the node for `SignedTransaction` verification.
 * preimage = nonce_le || sender(32) || bincode(payload) || bincode(access_list)
 */
export function signableTransactionHash(tx) {
    const preimage = concatBytes(writeU64Le(tx.nonce), tx.sender, encodeTransactionPayload(tx.payload), encodeAccessList(tx.accessList.read, tx.accessList.write));
    return blake3(preimage);
}
