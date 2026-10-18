/**
 * High-level transaction construction + Ed25519 signing (browser/Node).
 * Produces hex `SignedTransaction` for `boing_submitTransaction`.
 */
import * as ed25519 from '@noble/ed25519';
import { MAX_BATCH_CALLS, validateBatchCalls, encodeSignedTransaction, signableTransactionHash, signableTransactionHashV2, } from './bincode.js';
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
 * Contract call carrying native BOING (`TransactionPayload::ContractCallWithValue`). The value moves
 * to the contract before it runs and is refunded if the call fails. Nodes reject it before the
 * native-value activation height (testnet 1150). Default access list: sender + contract.
 */
export function buildContractCallWithValueTransaction(input) {
    const sender = validateHex32(input.senderHex).toLowerCase();
    const contract = validateHex32(input.contractHex).toLowerCase();
    const read = input.accessList?.read ?? [sender, contract];
    const write = input.accessList?.write ?? [sender, contract];
    return {
        nonce: input.nonce,
        sender: hexToAccountId(sender),
        payload: {
            kind: 'contractCallWithValue',
            contract: hexToAccountId(contract),
            calldata: input.calldata,
            value: input.value,
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
    const calls = input.calls.map((c) => {
        if (c.kind === 'transfer') {
            return { kind: 'transfer', to: hexToAccountId(validateHex32(c.toHex)), amount: c.amount };
        }
        const contract = hexToAccountId(validateHex32(c.contractHex));
        return c.kind === 'contractCallWithValue'
            ? { kind: 'contractCallWithValue', contract, calldata: c.calldata, value: c.value }
            : { kind: 'contractCall', contract, calldata: c.calldata };
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
let defaultSigningChainId = null;
/**
 * Process default chain id for v2 signing (used when `SignOptions.chainId` is omitted).
 * `null` = legacy (pre-chain-id) signatures, which nodes reject after their v2 activation height.
 */
export function setDefaultSigningChainId(chainId) {
    defaultSigningChainId = chainId == null ? null : BigInt(chainId);
}
export function getDefaultSigningChainId() {
    return defaultSigningChainId;
}
/** Chain id a node advertises for v2 signing (`boing_health.tx_signing`), or `null` (older node). */
export async function fetchSigningChainId(client) {
    try {
        const h = await client.health();
        const ts = h.tx_signing;
        if (ts && ts.v2_supported && ts.chain_id != null)
            return BigInt(ts.chain_id);
    }
    catch {
        /* unreachable / old node: legacy */
    }
    return null;
}
const detected = new WeakMap();
/** Sets the default signing chain id from the node once (no-op when a default is already set). */
export async function configureSigningFromNode(client) {
    if (defaultSigningChainId != null)
        return defaultSigningChainId;
    if (!detected.has(client))
        detected.set(client, await fetchSigningChainId(client));
    const c = detected.get(client) ?? null;
    if (c != null)
        defaultSigningChainId = c;
    return c;
}
/** Digest to sign for `tx` under `opts` (v2 when a chain id is known, else legacy). */
export function signingHashFor(tx, opts) {
    const cid = opts && opts.chainId !== undefined ? opts.chainId : defaultSigningChainId;
    return cid == null ? signableTransactionHash(tx) : signableTransactionHashV2(tx, cid);
}
/** Like `signTransactionInput`, but first adopts the node's advertised chain id (v2) when `client` is given. */
export async function signTransactionInputForClient(client, tx, secretKey32) {
    if (client)
        await configureSigningFromNode(client);
    return signTransactionInput(tx, secretKey32);
}
export async function signTransactionInput(tx, secretKey32, opts) {
    if (secretKey32.length !== 32)
        throw new Error('secret key must be 32 bytes');
    const pub = await ed25519.getPublicKeyAsync(secretKey32);
    if (!bytesEqual(pub, tx.sender)) {
        throw new Error('tx.sender must equal the Ed25519 public key derived from the secret key');
    }
    const hash = signingHashFor(tx, opts);
    const sig = await ed25519.signAsync(hash, secretKey32);
    const raw = encodeSignedTransaction(tx, sig);
    return bytesToHex(raw);
}
/**
 * Sign with a custom async signer (e.g. hardware wallet or IPC to Boing Express).
 * Caller must ensure the signer corresponds to `tx.sender`; no verification here.
 */
export async function signTransactionInputWithSigner(tx, signHash, opts) {
    const hash = signingHashFor(tx, opts);
    const sig = await signHash(hash);
    if (sig.length !== 64)
        throw new Error('Ed25519 signature must be 64 bytes');
    return bytesToHex(encodeSignedTransaction(tx, sig));
}
