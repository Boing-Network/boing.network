/**
 * Extract **predicted contract deployment addresses** from **`boing_getBlockByHeight`** JSON
 * (matches `boing-node` / `boing_primitives` deploy address rules: nonce-derived and CREATE2, including
 * init-code CREATE2 double prediction).
 *
 * Use this in indexers that need a **chain-wide contract deploy feed** (not DEX-scoped discovery).
 */
import { blake3 } from '@noble/hashes/blake3';
import { encodeTransaction } from './bincode.js';
import { predictCreate2ContractAddress, predictNonceDerivedContractAddress } from './create2.js';
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** Leading deploy init-code marker — matches `CONTRACT_DEPLOY_INIT_CODE_MARKER` in `boing_primitives`. */
export const CONTRACT_DEPLOY_INIT_CODE_MARKER = 0xfd;
function contractDeployUsesInitCode(bytecode) {
    return bytecode.length > 0 && bytecode[0] === CONTRACT_DEPLOY_INIT_CODE_MARKER;
}
function contractDeployInitBody(bytecode) {
    return contractDeployUsesInitCode(bytecode) ? bytecode.subarray(1) : bytecode;
}
function u8FromJsonAccount(v) {
    if (v instanceof Uint8Array && v.length === 32)
        return v;
    if (Array.isArray(v) && v.length === 32 && v.every((x) => typeof x === 'number')) {
        return Uint8Array.from(v);
    }
    if (typeof v === 'string') {
        return hexToBytes(validateHex32(v.trim()));
    }
    throw new Error('Expected 32-byte account (hex string or 32-element byte array)');
}
function u8FromJsonByteVec(v) {
    if (v instanceof Uint8Array)
        return v;
    if (Array.isArray(v) && v.every((x) => typeof x === 'number')) {
        return Uint8Array.from(v);
    }
    if (typeof v === 'string' && v.startsWith('0x')) {
        return hexToBytes(ensureEvenHexBytes(v));
    }
    throw new Error('Expected bytecode as u8 array or hex string');
}
function ensureEvenHexBytes(h) {
    const t = h.trim();
    if (!/^0x[0-9a-f]*$/i.test(t))
        throw new Error('invalid hex');
    return t.length % 2 === 0 ? t : `0x0${t.slice(2)}`;
}
function optSaltFromJson(v) {
    if (v == null)
        return null;
    if (Array.isArray(v) && v.length === 32)
        return Uint8Array.from(v);
    return null;
}
function rpcAccessListToBincode(access) {
    if (!access || typeof access !== 'object')
        throw new Error('access_list object expected');
    const o = access;
    const readRaw = Array.isArray(o.read) ? o.read : [];
    const writeRaw = Array.isArray(o.write) ? o.write : [];
    return {
        read: readRaw.map((x) => u8FromJsonAccount(x)),
        write: writeRaw.map((x) => u8FromJsonAccount(x)),
    };
}
/**
 * Map a **`Transaction`** object from **`boing_getBlockByHeight`** JSON into {@link TransactionInput}
 * for **`encodeTransaction`** / tx id hashing.
 */
export function rpcTransactionJsonToTransactionInput(txJson) {
    if (!txJson || typeof txJson !== 'object')
        throw new Error('transaction object expected');
    const tx = txJson;
    const nonce = tx.nonce;
    if (typeof nonce !== 'number' && typeof nonce !== 'bigint')
        throw new Error('nonce');
    const nonceBi = typeof nonce === 'bigint' ? nonce : BigInt(nonce);
    const sender = u8FromJsonAccount(tx.sender);
    const accessList = rpcAccessListToBincode(tx.access_list);
    const payload = tx.payload;
    if (!payload || typeof payload !== 'object')
        throw new Error('payload object expected');
    const p = payload;
    const deployShape = (kind, obj) => {
        const bytecode = u8FromJsonByteVec(obj.bytecode);
        const salt = optSaltFromJson(obj.create2_salt);
        if (kind === 'contractDeploy')
            return { kind: 'contractDeploy', bytecode, create2Salt: salt };
        if (kind === 'contractDeployWithPurpose') {
            const pc = obj.purpose_category;
            if (typeof pc !== 'string')
                throw new Error('purpose_category');
            const dh = obj.description_hash;
            let descriptionHash = null;
            if (Array.isArray(dh) && dh.length > 0)
                descriptionHash = Uint8Array.from(dh);
            return { kind: 'contractDeployWithPurpose', bytecode, purposeCategory: pc, descriptionHash, create2Salt: salt };
        }
        const pc = obj.purpose_category;
        if (typeof pc !== 'string')
            throw new Error('purpose_category');
        const dh = obj.description_hash;
        let descriptionHash = null;
        if (Array.isArray(dh) && dh.length > 0)
            descriptionHash = Uint8Array.from(dh);
        const assetName = typeof obj.asset_name === 'string' ? obj.asset_name : null;
        const assetSymbol = typeof obj.asset_symbol === 'string' ? obj.asset_symbol : null;
        return {
            kind: 'contractDeployWithPurposeAndMetadata',
            bytecode,
            purposeCategory: pc,
            descriptionHash,
            assetName,
            assetSymbol,
            create2Salt: salt,
        };
    };
    let payloadInput;
    if ('ContractDeploy' in p && p.ContractDeploy && typeof p.ContractDeploy === 'object') {
        payloadInput = deployShape('contractDeploy', p.ContractDeploy);
    }
    else if ('ContractDeployWithPurpose' in p && p.ContractDeployWithPurpose && typeof p.ContractDeployWithPurpose === 'object') {
        payloadInput = deployShape('contractDeployWithPurpose', p.ContractDeployWithPurpose);
    }
    else if ('ContractDeployWithPurposeAndMetadata' in p &&
        p.ContractDeployWithPurposeAndMetadata &&
        typeof p.ContractDeployWithPurposeAndMetadata === 'object') {
        payloadInput = deployShape('contractDeployWithPurposeAndMetadata', p.ContractDeployWithPurposeAndMetadata);
    }
    else if ('Transfer' in p) {
        const t = p.Transfer;
        const amt = t.amount;
        const amount = typeof amt === 'bigint' ? amt : BigInt(String(amt));
        payloadInput = { kind: 'transfer', to: u8FromJsonAccount(t.to), amount };
    }
    else if ('ContractCall' in p) {
        const c = p.ContractCall;
        payloadInput = {
            kind: 'contractCall',
            contract: u8FromJsonAccount(c.contract),
            calldata: u8FromJsonByteVec(c.calldata),
        };
    }
    else if ('Bond' in p) {
        const b = p.Bond;
        const a = b.amount;
        payloadInput = { kind: 'bond', amount: typeof a === 'bigint' ? a : BigInt(String(a)) };
    }
    else if ('Unbond' in p) {
        const u = p.Unbond;
        const a = u.amount;
        payloadInput = { kind: 'unbond', amount: typeof a === 'bigint' ? a : BigInt(String(a)) };
    }
    else if ('ClaimUnbond' in p) {
        payloadInput = { kind: 'claimUnbond' };
    }
    else {
        throw new Error('Unsupported or missing transaction payload variant');
    }
    return { nonce: nonceBi, sender, payload: payloadInput, accessList };
}
/** `BLAKE3(bincode(Transaction))` hex — matches Rust `Transaction::id()`. */
export function transactionIdFromUnsignedRpcTransaction(txJson) {
    const input = rpcTransactionJsonToTransactionInput(txJson);
    return validateHex32(bytesToHex(blake3(encodeTransaction(input))));
}
function predictedContractHexes(bytecode, senderU8, deployNonce, salt) {
    const senderHex = validateHex32(bytesToHex(senderU8));
    if (salt != null && salt.length === 32) {
        const primary = predictCreate2ContractAddress(senderHex, salt, bytecode);
        if (contractDeployUsesInitCode(bytecode)) {
            const body = contractDeployInitBody(bytecode);
            const secondary = predictCreate2ContractAddress(senderHex, salt, body);
            return primary === secondary ? [primary] : [primary, secondary];
        }
        return [primary];
    }
    return [predictNonceDerivedContractAddress(senderHex, deployNonce)];
}
function payloadKindFromTx(txJson) {
    if (!txJson || typeof txJson !== 'object')
        return null;
    const p = txJson.payload;
    if (!p || typeof p !== 'object')
        return null;
    const o = p;
    if ('ContractDeploy' in o)
        return 'ContractDeploy';
    if ('ContractDeployWithPurpose' in o)
        return 'ContractDeployWithPurpose';
    if ('ContractDeployWithPurposeAndMetadata' in o)
        return 'ContractDeployWithPurposeAndMetadata';
    return null;
}
function optionalDeployMeta(txJson, kind) {
    if (!txJson || typeof txJson !== 'object')
        return {};
    const p = txJson.payload;
    if (!p || typeof p !== 'object')
        return {};
    const inner = kind === 'ContractDeploy'
        ? p.ContractDeploy
        : kind === 'ContractDeployWithPurpose'
            ? p.ContractDeployWithPurpose
            : p.ContractDeployWithPurposeAndMetadata;
    if (!inner || typeof inner !== 'object')
        return {};
    const purposeCategory = typeof inner.purpose_category === 'string' ? inner.purpose_category : undefined;
    const assetName = typeof inner.asset_name === 'string' ? inner.asset_name : undefined;
    const assetSymbol = typeof inner.asset_symbol === 'string' ? inner.asset_symbol : undefined;
    return { purposeCategory, assetName, assetSymbol };
}
/** Extract deploys from a full **`Block`**-shaped JSON object (`header.height`, `transactions`). */
export function extractUniversalContractDeploymentsFromBlock(block) {
    if (!block || typeof block !== 'object')
        return [];
    const b = block;
    const h = b.header?.height;
    const height = typeof h === 'number' && Number.isFinite(h) ? h : typeof h === 'string' ? parseInt(h, 10) : NaN;
    if (!Number.isFinite(height))
        return [];
    const txs = Array.isArray(b.transactions) ? b.transactions : [];
    return extractUniversalContractDeploymentsFromBlockJson(height, txs);
}
export function extractUniversalContractDeploymentsFromBlockJson(blockHeight, transactions) {
    const out = [];
    transactions.forEach((txJson, txIndex) => {
        const kind = payloadKindFromTx(txJson);
        if (!kind)
            return;
        let input;
        let txId;
        try {
            input = rpcTransactionJsonToTransactionInput(txJson);
            txId = validateHex32(bytesToHex(blake3(encodeTransaction(input))));
        }
        catch {
            return;
        }
        const deployPayload = input.payload;
        if (deployPayload.kind !== 'contractDeploy' &&
            deployPayload.kind !== 'contractDeployWithPurpose' &&
            deployPayload.kind !== 'contractDeployWithPurposeAndMetadata') {
            return;
        }
        const bytecode = deployPayload.bytecode;
        const salt = deployPayload.create2Salt ?? null;
        const senderHex = validateHex32(bytesToHex(input.sender));
        const meta = optionalDeployMeta(txJson, kind);
        const addresses = predictedContractHexes(bytecode, input.sender, input.nonce, salt);
        const seen = new Set();
        for (const contractHex of addresses) {
            const low = contractHex.toLowerCase();
            if (seen.has(low))
                continue;
            seen.add(low);
            out.push({
                contractHex: low,
                blockHeight,
                txIndex,
                txIdHex: txId,
                senderHex,
                payloadKind: kind,
                ...meta,
            });
        }
    });
    return out;
}
