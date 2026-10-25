/**
 * bincode 1.3 layout matching `crates/boing-primitives` (serde derive).
 * Used for `Transaction`, `SignedTransaction`, and signable hash preimage.
 * @see docs/BOING-SIGNED-TRANSACTION-ENCODING.md
 */
import { blake3 } from '@noble/hashes/blake3';
import { bytesToHex, ensureHex, hexToBytes } from './hex.js';
/** Payload variants — discriminant u32 LE must match Rust enum declaration order. */
export const PayloadVariant = {
    Transfer: 0,
    ContractCall: 1,
    ContractDeploy: 2,
    ContractDeployWithPurpose: 3,
    ContractDeployWithPurposeAndMetadata: 4,
    Bond: 5,
    Unbond: 6,
    ClaimUnbond: 7,
    QaPoolVote: 8,
    /** Atomic batch of transfers / contract calls under one signature + nonce (node ≥ batch release). */
    Batch: 9,
    /** Open validator registration (owner signs tx; node key signs the registration proof). */
    ValidatorRegister: 10,
    ValidatorDeregister: 11,
    ValidatorUnjail: 12,
    /** Name registry (`label.boing`), see `boingNames.ts`. */
    NameCommit: 13,
    NameRegister: 14,
    NameRenew: 15,
    NameTransfer: 16,
    NameSetTarget: 17,
    NameSetPrimary: 18,
    NameRelease: 19,
    /** Contract call carrying native BOING (`value`, base units). Testnet: from height 1150. */
    ContractCallWithValue: 20,
    /** L1 media blob publish (`docs/BOING-L1-MEDIA-BLOBS.md`). Phase 2 cap 1 MiB. */
    PutBlob: 21,
    /** Announce blob id + size without inline bytes (P2P / sidecar data plane). */
    BlobAnnounce: 22,
    /** Extend media blob pin horizon (Phase 5). */
    RenewMediaBlobPin: 23,
    /** Pay for one DAS chunk sample (Phase 5 market scaffold). */
    PayBlobSample: 24,
    /** Register on-chain sample offer (Phase 7). */
    RegisterBlobSampleOffer: 25,
    /** Settle sample match against a registered provider (Phase 7). */
    SettleBlobSampleMatch: 26,
    /** Cancel sample offer and refund escrow (Phase 9). */
    CancelBlobSampleOffer: 27,
    /** Challenge sample serve availability (Phase 10). */
    ChallengeBlobSampleServe: 28,
    /** Respond to serve challenge with DAS sample (Phase 10). */
    RespondBlobSampleServe: 29,
    /** Submit consensus slash appeal (Phase 14). */
    SubmitSlashAppeal: 30,
    /** Resolve consensus slash appeal (Phase 14; unattested). */
    ResolveSlashAppeal: 31,
    /** Attested resolve with Ed25519 ballot (Phase 18). */
    ResolveSlashAppealAttested: 32,
    /** BLS aggregate resolve with self-contained pubkeys (Phase 30; not consensus-admitted). */
    ResolveSlashAppealBlsAggregate: 33,
    /** Grant media-blob download ACL (Phase 34; publisher-only). */
    GrantBlobDownload: 34,
    /** Revoke media-blob download ACL (Phase 34; publisher-only). */
    RevokeBlobDownload: 35,
};
/** `BlobRetention` discriminants (Rust `boing_primitives::BlobRetention`). */
export const BlobRetentionVariant = {
    Prunable: 0,
    Forever: 1,
};
/** `BatchCall` discriminants (Rust `boing_primitives::BatchCall`). */
export const BatchCallVariant = {
    Transfer: 0,
    ContractCall: 1,
    /** Contract call step carrying native BOING (testnet height 1150+). */
    ContractCallWithValue: 2,
};
/** Protocol cap on steps in one batch (`boing_primitives::MAX_BATCH_CALLS`). */
export const MAX_BATCH_CALLS = 16;
/** Protocol cap on summed contract-call calldata in one batch (`MAX_BATCH_CALLDATA_BYTES`). */
export const MAX_BATCH_CALLDATA_BYTES = 64 * 1024;
const U128_LIMIT = 1n << 128n;
function checkValue(v) {
    if (v < 0n || v >= U128_LIMIT)
        throw new Error('value must fit u128');
}
/** Throws with the same reasons the node uses (`BatchValidationError`). */
export function validateBatchCalls(calls) {
    if (calls.length === 0)
        throw new Error('batch has no calls');
    if (calls.length > MAX_BATCH_CALLS) {
        throw new Error(`batch has ${calls.length} calls (max ${MAX_BATCH_CALLS})`);
    }
    let calldata = 0;
    let total = 0n;
    for (const c of calls) {
        if (c.kind === 'transfer') {
            if (c.amount < 0n)
                throw new Error('batch transfer amount must be >= 0');
            total += c.amount;
        }
        else {
            calldata += c.calldata.length;
            if (c.kind === 'contractCallWithValue') {
                checkValue(c.value);
                total += c.value;
            }
        }
    }
    if (total >= U128_LIMIT)
        throw new Error('batch transfer total overflows u128');
    if (calldata > MAX_BATCH_CALLDATA_BYTES) {
        throw new Error(`batch calldata is ${calldata} bytes (max ${MAX_BATCH_CALLDATA_BYTES})`);
    }
}
function encodeBatchCall(c) {
    if (c.kind === 'transfer') {
        if (c.to.length !== 32)
            throw new Error('batch transfer `to` must be 32 bytes');
        return concatBytes(writeU32Le(BatchCallVariant.Transfer), c.to, writeU128Le(c.amount));
    }
    if (c.contract.length !== 32)
        throw new Error('batch contract must be 32 bytes');
    if (c.kind === 'contractCallWithValue') {
        checkValue(c.value);
        return concatBytes(writeU32Le(BatchCallVariant.ContractCallWithValue), c.contract, encodeByteVec(c.calldata), writeU128Le(c.value));
    }
    return concatBytes(writeU32Le(BatchCallVariant.ContractCall), c.contract, encodeByteVec(c.calldata));
}
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
function need32(b, what) {
    if (b.length !== 32)
        throw new Error(`${what} must be 32 bytes`);
    return b;
}
function yearsByte(years) {
    if (!Number.isInteger(years) || years < 1 || years > 10)
        throw new Error('years must be 1-10');
    return Uint8Array.of(years);
}
export function encodeTransactionPayload(payload) {
    switch (payload.kind) {
        case 'transfer':
            return concatBytes(writeU32Le(PayloadVariant.Transfer), payload.to, writeU128Le(payload.amount));
        case 'contractCall':
            return concatBytes(writeU32Le(PayloadVariant.ContractCall), payload.contract, encodeByteVec(payload.calldata));
        case 'contractCallWithValue':
            checkValue(payload.value);
            return concatBytes(writeU32Le(PayloadVariant.ContractCallWithValue), need32(payload.contract, 'contract'), encodeByteVec(payload.calldata), writeU128Le(payload.value));
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
        case 'claimUnbond':
            return writeU32Le(PayloadVariant.ClaimUnbond);
        case 'qaPoolVote': {
            const voteDisc = payload.vote === 'allow' ? 0 : payload.vote === 'reject' ? 1 : 2;
            if (payload.subject.length !== 32) {
                throw new Error('qaPoolVote subject must be 32 bytes');
            }
            return concatBytes(writeU32Le(PayloadVariant.QaPoolVote), payload.subject, writeU32Le(voteDisc));
        }
        case 'batch':
            validateBatchCalls(payload.calls);
            return concatBytes(writeU32Le(PayloadVariant.Batch), writeU64Le(BigInt(payload.calls.length)), ...payload.calls.map(encodeBatchCall));
        case 'validatorRegister':
            if (payload.nodeId.length !== 32)
                throw new Error('validator nodeId must be 32 bytes');
            if (payload.nodeSignature.length !== 64)
                throw new Error('validator nodeSignature must be 64 bytes');
            return concatBytes(writeU32Le(PayloadVariant.ValidatorRegister), payload.nodeId, encodeByteVec(payload.nodeSignature));
        case 'validatorDeregister':
            if (payload.nodeId.length !== 32)
                throw new Error('validator nodeId must be 32 bytes');
            return concatBytes(writeU32Le(PayloadVariant.ValidatorDeregister), payload.nodeId);
        case 'validatorUnjail':
            if (payload.nodeId.length !== 32)
                throw new Error('validator nodeId must be 32 bytes');
            return concatBytes(writeU32Le(PayloadVariant.ValidatorUnjail), payload.nodeId);
        case 'nameCommit':
            return concatBytes(writeU32Le(PayloadVariant.NameCommit), need32(payload.commitment, 'commitment'));
        case 'nameRegister':
            return concatBytes(writeU32Le(PayloadVariant.NameRegister), encodeBincodeString(payload.name), need32(payload.salt, 'salt'), need32(payload.target, 'target'), yearsByte(payload.years));
        case 'nameRenew':
            return concatBytes(writeU32Le(PayloadVariant.NameRenew), encodeBincodeString(payload.name), yearsByte(payload.years));
        case 'nameTransfer':
            return concatBytes(writeU32Le(PayloadVariant.NameTransfer), encodeBincodeString(payload.name), need32(payload.newOwner, 'newOwner'));
        case 'nameSetTarget':
            return concatBytes(writeU32Le(PayloadVariant.NameSetTarget), encodeBincodeString(payload.name), need32(payload.target, 'target'));
        case 'nameSetPrimary':
            return concatBytes(writeU32Le(PayloadVariant.NameSetPrimary), encodeOptionString(payload.name));
        case 'nameRelease':
            return concatBytes(writeU32Le(PayloadVariant.NameRelease), encodeBincodeString(payload.name));
        case 'putBlob': {
            const retention = payload.retention.kind === 'forever'
                ? writeU32Le(BlobRetentionVariant.Forever)
                : concatBytes(writeU32Le(BlobRetentionVariant.Prunable), writeU64Le(payload.retention.ttlBlocks));
            return concatBytes(writeU32Le(PayloadVariant.PutBlob), retention, encodeBincodeString(payload.contentType ?? ''), encodeByteVec(payload.data));
        }
        case 'blobAnnounce': {
            const retention = payload.retention.kind === 'forever'
                ? writeU32Le(BlobRetentionVariant.Forever)
                : concatBytes(writeU32Le(BlobRetentionVariant.Prunable), writeU64Le(payload.retention.ttlBlocks));
            return concatBytes(writeU32Le(PayloadVariant.BlobAnnounce), need32(payload.blobId, 'blobId'), writeU32Le(payload.size >>> 0), retention, encodeBincodeString(payload.contentType ?? ''));
        }
        case 'renewMediaBlobPin':
            return concatBytes(writeU32Le(PayloadVariant.RenewMediaBlobPin), need32(payload.blobId, 'blobId'), writeU32Le(payload.size >>> 0), writeU64Le(payload.extendBlocks));
        case 'payBlobSample':
            return concatBytes(writeU32Le(PayloadVariant.PayBlobSample), need32(payload.blobId, 'blobId'), writeU32Le(payload.chunkIndex >>> 0), writeU32Le(payload.chunkCount >>> 0));
        case 'registerBlobSampleOffer':
            return concatBytes(writeU32Le(PayloadVariant.RegisterBlobSampleOffer), need32(payload.blobId, 'blobId'), writeU32Le(payload.chunkIndex >>> 0), writeU32Le(payload.chunkCount >>> 0), writeU128Le(payload.askFee), writeU64Le(payload.expiresAtHeight));
        case 'settleBlobSampleMatch':
            return concatBytes(writeU32Le(PayloadVariant.SettleBlobSampleMatch), need32(payload.blobId, 'blobId'), writeU32Le(payload.chunkIndex >>> 0), writeU32Le(payload.chunkCount >>> 0), need32(payload.provider, 'provider'));
        case 'cancelBlobSampleOffer':
            return concatBytes(writeU32Le(PayloadVariant.CancelBlobSampleOffer), need32(payload.blobId, 'blobId'), writeU32Le(payload.chunkIndex >>> 0));
        case 'challengeBlobSampleServe':
            return concatBytes(writeU32Le(PayloadVariant.ChallengeBlobSampleServe), need32(payload.blobId, 'blobId'), writeU32Le(payload.chunkIndex >>> 0), writeU32Le(payload.chunkCount >>> 0), need32(payload.provider, 'provider'));
        case 'respondBlobSampleServe': {
            const sibs = payload.siblings;
            const parts = [
                writeU32Le(PayloadVariant.RespondBlobSampleServe),
                need32(payload.blobId, 'blobId'),
                writeU32Le(payload.chunkIndex >>> 0),
                writeU32Le(payload.chunkCount >>> 0),
                need32(payload.challenger, 'challenger'),
                need32(payload.chunkHash, 'chunkHash'),
                need32(payload.chunksRoot, 'chunksRoot'),
                writeU64Le(BigInt(sibs.length)),
            ];
            for (const s of sibs)
                parts.push(need32(s, 'sibling'));
            return concatBytes(...parts);
        }
        case 'submitSlashAppeal':
            return concatBytes(writeU32Le(PayloadVariant.SubmitSlashAppeal), writeU64Le(payload.slashId), writeU64Le(BigInt(payload.evidence.length)), payload.evidence);
        case 'resolveSlashAppeal':
            return concatBytes(writeU32Le(PayloadVariant.ResolveSlashAppeal), writeU64Le(payload.appealId), Uint8Array.of(payload.approved ? 1 : 0));
        case 'resolveSlashAppealAttested': {
            if (payload.ballotSignature.length !== 64) {
                throw new Error('resolveSlashAppealAttested ballotSignature must be 64 bytes');
            }
            return concatBytes(writeU32Le(PayloadVariant.ResolveSlashAppealAttested), writeU64Le(payload.appealId), Uint8Array.of(payload.approved ? 1 : 0), payload.ballotSignature);
        }
        case 'resolveSlashAppealBlsAggregate': {
            if (payload.aggregateSignature.length !== 96) {
                throw new Error('resolveSlashAppealBlsAggregate aggregateSignature must be 96 bytes');
            }
            const parts = [
                writeU32Le(PayloadVariant.ResolveSlashAppealBlsAggregate),
                writeU64Le(payload.appealId),
                Uint8Array.of(payload.approved ? 1 : 0),
                writeU64Le(BigInt(payload.signerBitset.length)),
                payload.signerBitset,
                payload.aggregateSignature,
                writeU64Le(BigInt(payload.signerBlsPubkeys.length)),
            ];
            for (const pk of payload.signerBlsPubkeys) {
                if (pk.length !== 48) {
                    throw new Error(`resolveSlashAppealBlsAggregate pubkey must be 48 bytes, got ${pk.length}`);
                }
                parts.push(pk);
            }
            return concatBytes(...parts);
        }
        case 'grantBlobDownload':
            return concatBytes(writeU32Le(PayloadVariant.GrantBlobDownload), need32(payload.blobId, 'blobId'), need32(payload.grantee, 'grantee'));
        case 'revokeBlobDownload':
            return concatBytes(writeU32Le(PayloadVariant.RevokeBlobDownload), need32(payload.blobId, 'blobId'), need32(payload.grantee, 'grantee'));
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
    const tip = tx.priorityTip ?? 0n;
    const base = concatBytes(encodeTransaction(tx), encodeSignature(signature64));
    // Match Rust `skip_serializing_if = is_zero_u128`: tip omitted when zero.
    if (tip === 0n)
        return base;
    return concatBytes(base, writeU128Le(tip));
}
/**
 * BLAKE3 hash signed by the node for `SignedTransaction` verification.
 * preimage = nonce_le || sender(32) || bincode(payload) || bincode(access_list)
 */
export function signableTransactionHash(tx) {
    const preimage = concatBytes(writeU64Le(tx.nonce), tx.sender, encodeTransactionPayload(tx.payload), encodeAccessList(tx.accessList.read, tx.accessList.write));
    return blake3(preimage);
}
/** Domain tag of the v2 (chain-bound) signing preimage. */
export const TX_SIGNING_DOMAIN_V2 = new TextEncoder().encode('BOING_TX_V2');
/** Domain tag when a non-zero priority tip is bound into the signature. */
export const TX_SIGNING_DOMAIN_PRIORITY = new TextEncoder().encode('BOING_TX_PRIORITY');
/**
 * v2 signable hash (security fix #2, cross-chain replay): BLAKE3( "BOING_TX_V2" || chain_id_le(8) ||
 * nonce_le || sender(32) || bincode(payload) || bincode(access_list) ). Matches Rust
 * `signable_transaction_hash_v2`. Required by nodes after the network's v2 activation height
 * (testnet 6913: height 900; mainnet 6914: genesis).
 */
export function signableTransactionHashV2(tx, chainId) {
    const preimage = concatBytes(TX_SIGNING_DOMAIN_V2, writeU64Le(BigInt(chainId)), writeU64Le(tx.nonce), tx.sender, encodeTransactionPayload(tx.payload), encodeAccessList(tx.accessList.read, tx.accessList.write));
    return blake3(preimage);
}
/**
 * Signable digest including an optional priority tip. Tip `0` == [`signableTransactionHashV2`].
 * Non-zero: BLAKE3(`BOING_TX_PRIORITY` || chain_id || nonce || sender || payload || access_list || tip_le16).
 */
export function signableTransactionHashWithTip(tx, chainId, priorityTip = tx.priorityTip ?? 0n) {
    if (priorityTip === 0n)
        return signableTransactionHashV2(tx, chainId);
    const preimage = concatBytes(TX_SIGNING_DOMAIN_PRIORITY, writeU64Le(BigInt(chainId)), writeU64Le(tx.nonce), tx.sender, encodeTransactionPayload(tx.payload), encodeAccessList(tx.accessList.read, tx.accessList.write), writeU128Le(priorityTip));
    return blake3(preimage);
}
const SIGNED_TX_SIGNATURE_TRAIL_BYTES = 8 + 64;
const SIGNED_TX_TIP_BYTES = 16;
/**
 * Derives the mempool / receipt **`tx_id`** (32-byte BLAKE3) from **`0x` + bincode(`SignedTransaction`)** hex
 * returned by Boing Express after `boing_signTransaction`. Matches **`Transaction::id()`** / **`boing_getTransactionReceipt`** param
 * per [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md) (signable body excludes the trailing serde `Signature` block
 * and optional priority tip).
 *
 * @throws if hex is invalid or the trailing signature length field is not 64
 */
export function transactionIdFromSignedTransactionHex(signedTxHex) {
    const bytes = hexToBytes(ensureHex(signedTxHex));
    if (bytes.length < SIGNED_TX_SIGNATURE_TRAIL_BYTES + 1) {
        throw new Error('Signed transaction bytes too short to derive tx_id');
    }
    const tryOffset = (sigLenOffset) => {
        if (sigLenOffset < 1)
            return null;
        const dv = new DataView(bytes.buffer, bytes.byteOffset + sigLenOffset, 8);
        const sigLen = dv.getBigUint64(0, true);
        if (sigLen !== 64n)
            return null;
        const txBody = bytes.subarray(0, sigLenOffset);
        return bytesToHex(blake3(txBody));
    };
    // Prefer tipped layout (sig then tip) when the trailing 16 bytes look like a tip extension.
    if (bytes.length >= SIGNED_TX_SIGNATURE_TRAIL_BYTES + SIGNED_TX_TIP_BYTES + 1) {
        const tipped = tryOffset(bytes.length - SIGNED_TX_SIGNATURE_TRAIL_BYTES - SIGNED_TX_TIP_BYTES);
        if (tipped)
            return tipped;
    }
    const untipped = tryOffset(bytes.length - SIGNED_TX_SIGNATURE_TRAIL_BYTES);
    if (!untipped) {
        throw new Error('Unexpected serde signature length field (expected 64)');
    }
    return untipped;
}
