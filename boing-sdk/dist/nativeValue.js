import { validateBatchCalls } from './bincode.js';
import { accountIdToHex, bytesToHex, hexToAccountId, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataSelectorLastByte, calldataU128BeWord, concatCalldata } from './calldata.js';
import { encodeReferenceApproveCalldata } from './referenceToken.js';
import { boingPoolEffectiveFeeBps, boingPoolMinOutWithSlippage, encodeBoingPoolV8SwapCalldata, quoteBoingPoolAmountOut, } from './boingPool.js';
import { WRAPPED_BOING_BYTECODE_HEX } from './defaultWrappedBoingBytecodeHex.js';
import { BOING_TESTNET_CHAIN_ID_DECIMAL } from './chainIds.js';
export { WRAPPED_BOING_BYTECODE_HEX };
/** Testnet (6913) activation height of the native-value rules (`BOING_TESTNET_NATIVE_VALUE_FROM`). */
export const BOING_TESTNET_NATIVE_VALUE_FROM = 1150;
/**
 * `true` when value-carrying calls are accepted in the block at `height` (pass tip + 1 for a new tx).
 * Testnet: from 1150. Other chains (devnets, chains without history) have the rules from genesis.
 */
export function nativeValueActiveAt(chainId, height) {
    if (chainId != null && Number(chainId) === BOING_TESTNET_CHAIN_ID_DECIMAL) {
        return BigInt(height) >= BigInt(BOING_TESTNET_NATIVE_VALUE_FROM);
    }
    return true;
}
export const WBOING_SELECTOR_DEPOSIT = 0x20;
export const WBOING_SELECTOR_WITHDRAW = 0x21;
export const WBOING_SELECTOR_WITHDRAW_TO = 0x22;
export const WBOING_SELECTOR_WITHDRAW_ALL = 0x23;
export const WBOING_SELECTOR_TOTAL_SUPPLY = 0x24;
export const WBOING_SELECTOR_BALANCE_OF = 0x25;
export const WBOING_SELECTOR_DECIMALS = 0x26;
export const WBOING_DECIMALS = 18;
/** Log topic0 of a deposit (`"BOING_WBOING_DEPOSIT"` right-padded to 32 bytes). */
export const WBOING_TOPIC_DEPOSIT_HEX = asciiTopic('BOING_WBOING_DEPOSIT');
/** Log topic0 of a withdrawal. */
export const WBOING_TOPIC_WITHDRAW_HEX = asciiTopic('BOING_WBOING_WITHDRAW');
/** Storage key of the wBOING total supply. */
export const WBOING_TOTAL_SUPPLY_KEY_HEX = asciiTopic('BOING_WBOING_TOTAL_SUPPLY');
function asciiTopic(s) {
    const b = new Uint8Array(32);
    b.set(new TextEncoder().encode(s));
    return bytesToHex(b);
}
export const encodeWboingDepositCalldata = () => calldataSelectorLastByte(WBOING_SELECTOR_DEPOSIT);
export const encodeWboingWithdrawCalldata = (amount) => concatCalldata([calldataSelectorLastByte(WBOING_SELECTOR_WITHDRAW), calldataU128BeWord(amount)]);
export const encodeWboingWithdrawToCalldata = (amount, recipientHex) => concatCalldata([calldataSelectorLastByte(WBOING_SELECTOR_WITHDRAW_TO), calldataU128BeWord(amount), calldataAccountIdWord(recipientHex)]);
export const encodeWboingWithdrawAllCalldata = () => calldataSelectorLastByte(WBOING_SELECTOR_WITHDRAW_ALL);
export const encodeWboingTotalSupplyCalldata = () => calldataSelectorLastByte(WBOING_SELECTOR_TOTAL_SUPPLY);
export const encodeWboingBalanceOfCalldata = (holderHex) => concatCalldata([calldataSelectorLastByte(WBOING_SELECTOR_BALANCE_OF), calldataAccountIdWord(holderHex)]);
const U128 = 1n << 128n;
const lc = (h) => validateHex32(h).toLowerCase();
function accessFor(ids) {
    const uniq = [...new Set(ids.map(lc))];
    const acc = uniq.map(hexToAccountId);
    return { read: acc, write: acc.map((a) => a.slice()) };
}
/** `deposit` with `amount` native BOING: mints `amount` wBOING to the sender (one tx). */
export function buildWrapBoingTransaction(input) {
    if (input.amount <= 0n || input.amount >= U128)
        throw new RangeError('amount must be in 1..2^128-1');
    return {
        nonce: input.nonce,
        sender: hexToAccountId(lc(input.senderHex)),
        payload: { kind: 'contractCallWithValue', contract: hexToAccountId(lc(input.wboingHex)), calldata: encodeWboingDepositCalldata(), value: input.amount },
        accessList: accessFor([input.senderHex, input.wboingHex]),
    };
}
/** `withdraw(amount)` (or `withdraw_all` when `amount` is `'all'`): burns wBOING and pays native BOING. */
export function buildUnwrapBoingTransaction(input) {
    const calldata = input.amount === 'all' ? encodeWboingWithdrawAllCalldata() : encodeWboingWithdrawCalldata(input.amount);
    return {
        nonce: input.nonce,
        sender: hexToAccountId(lc(input.senderHex)),
        payload: { kind: 'contractCall', contract: hexToAccountId(lc(input.wboingHex)), calldata },
        accessList: accessFor([input.senderHex, input.wboingHex]),
    };
}
/**
 * Plan a native BOING swap against a v10 custody pool whose one side is wBOING. Pure (no RPC): pass the
 * pool state (reserves, tokens, stored fee) and the sender's next nonce. The batch either wraps the input
 * (`deposit` with value, `approve(pool)`, `swap`) or unwraps the output (`approve(pool)`, `swap`,
 * `withdraw_all`). `withdraw_all` also unwraps wBOING the sender already held.
 */
export function planBoingNativeSwap(input) {
    if (input.amountIn <= 0n)
        throw new Error('amountIn must be > 0');
    if (input.slippageBps < 0n || input.slippageBps >= 10000n)
        throw new Error('slippageBps must be in [0, 10000)');
    const w = lc(input.wboingHex);
    const tin = input.assetIn === 'native' ? w : lc(input.assetIn);
    const tout = input.assetOut === 'native' ? w : lc(input.assetOut);
    if ((input.assetIn === 'native') === (input.assetOut === 'native'))
        throw new Error('exactly one side must be native BOING');
    const ta = input.state.tokenAHex.toLowerCase().slice(-64);
    const tb = input.state.tokenBHex.toLowerCase().slice(-64);
    let direction;
    if (tin.slice(-64) === ta && tout.slice(-64) === tb)
        direction = 0;
    else if (tin.slice(-64) === tb && tout.slice(-64) === ta)
        direction = 1;
    else
        throw new Error('pool does not pair these assets');
    const version = input.poolVersion ?? 10;
    const [ri, ro] = direction === 0 ? [input.state.reserveA, input.state.reserveB] : [input.state.reserveB, input.state.reserveA];
    const feeBps = boingPoolEffectiveFeeBps(version, input.state.storedFeeBps);
    const amountOut = quoteBoingPoolAmountOut(version, ri, ro, input.amountIn, feeBps);
    if (amountOut <= 0n)
        throw new Error('quote is 0 (amount too small or pool empty)');
    let minOut = boingPoolMinOutWithSlippage(amountOut, input.slippageBps);
    if (minOut > amountOut)
        minOut = amountOut;
    const pool = lc(input.poolHex);
    const swapCd = encodeBoingPoolV8SwapCalldata(BigInt(direction), input.amountIn, minOut, input.deadlineHeight ?? 0n);
    const approveCd = encodeReferenceApproveCalldata(pool, input.amountIn);
    const c = (h) => hexToAccountId(h);
    let calls;
    let steps;
    const mode = input.assetIn === 'native' ? 'wrap' : 'unwrap';
    if (mode === 'wrap') {
        calls = [
            { kind: 'contractCallWithValue', contract: c(w), calldata: encodeWboingDepositCalldata(), value: input.amountIn },
            { kind: 'contractCall', contract: c(w), calldata: approveCd },
            { kind: 'contractCall', contract: c(pool), calldata: swapCd },
        ];
        steps = ['Wrap BOING', 'Approve pool', 'Swap'];
    }
    else {
        calls = [
            { kind: 'contractCall', contract: c(tin), calldata: approveCd },
            { kind: 'contractCall', contract: c(pool), calldata: swapCd },
            { kind: 'contractCall', contract: c(w), calldata: encodeWboingWithdrawAllCalldata() },
        ];
        steps = ['Approve pool', 'Swap', 'Unwrap to BOING'];
    }
    validateBatchCalls(calls);
    const tx = {
        nonce: input.nonce,
        sender: c(lc(input.senderHex)),
        payload: { kind: 'batch', calls },
        accessList: accessFor([input.senderHex, w, pool, tin, tout]),
    };
    return { direction, amountIn: input.amountIn, amountOut, minOut, feeBps, mode, value: mode === 'wrap' ? input.amountIn : 0n, tx, steps };
}
/**
 * Boing Express request object for a `contractCall`, `contractCallWithValue` or `batch` `TransactionInput`
 * (hex fields, snake_case). Value-carrying steps get `value` (decimal base units).
 */
export function boingTxToExpressRequest(tx) {
    const access_list = { read: tx.accessList.read.map(accountIdToHex), write: tx.accessList.write.map(accountIdToHex) };
    const p = tx.payload;
    if (p.kind === 'contractCall')
        return { type: 'contract_call', contract: accountIdToHex(p.contract), calldata: bytesToHex(p.calldata), access_list };
    if (p.kind === 'contractCallWithValue') {
        return { type: 'contract_call', contract: accountIdToHex(p.contract), calldata: bytesToHex(p.calldata), value: p.value.toString(), access_list };
    }
    if (p.kind === 'batch') {
        const calls = p.calls.map((s) => s.kind === 'transfer'
            ? { type: 'transfer', to: accountIdToHex(s.to), amount: s.amount.toString() }
            : s.kind === 'contractCallWithValue'
                ? { type: 'contract_call', contract: accountIdToHex(s.contract), calldata: bytesToHex(s.calldata), value: s.value.toString() }
                : { type: 'contract_call', contract: accountIdToHex(s.contract), calldata: bytesToHex(s.calldata) });
        return { type: 'batch', calls, access_list };
    }
    throw new Error(`unsupported payload for Express: ${p.kind}`);
}
