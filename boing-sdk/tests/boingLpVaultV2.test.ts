import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import {
  boingLpVaultV2ShareKeyHex,
  buildBoingLpVaultV2DepositFlow,
  depositBoingLpVaultV2,
  encodeBoingLpVaultV2ConfigureCalldata,
  encodeBoingLpVaultV2DepositCalldata,
  encodeBoingLpVaultV2TransferSharesCalldata,
  encodeBoingLpVaultV2WithdrawCalldata,
  quoteBoingLpVaultV2Deposit,
  quoteBoingLpVaultV2Withdraw,
  withdrawBoingLpVaultV2,
} from '../src/boingLpVaultV2.js';
import { bytesToHex, hexToBytes } from '../src/hex.js';

const V = JSON.parse(readFileSync(new URL('./fixtures/pool-cli-vectors.json', import.meta.url), 'utf8'));
const L = V.lpVaultV2;

describe('LP vault v2 calldata matches the CLI / VM encoders', () => {
  it('configure / deposit / withdraw / transfer_shares / share key', () => {
    expect(bytesToHex(encodeBoingLpVaultV2ConfigureCalldata(V.poolHex, V.tokenAHex, V.tokenBHex))).toBe(L.configure);
    expect(bytesToHex(encodeBoingLpVaultV2DepositCalldata(10_000n, 20_000n, 9_000n, 77n))).toBe(L.deposit);
    expect(bytesToHex(encodeBoingLpVaultV2WithdrawCalldata(4_000n, 3_000n, 6_000n))).toBe(L.withdraw);
    expect(bytesToHex(encodeBoingLpVaultV2TransferSharesCalldata('0x' + '0c'.repeat(32), 1_234n))).toBe(L.transferShares);
    expect(boingLpVaultV2ShareKeyHex(V.senderHex)).toBe(L.shareKeyHex);
  });
});

describe('quotes', () => {
  const s = { reserveA: 100_000n, reserveB: 200_000n, totalLp: 100_000n };
  it('balanced deposit rounds amountB up and bounds minShares', () => {
    const q = quoteBoingLpVaultV2Deposit(s, 1_001n, 100n);
    expect(q.amountB).toBe(2_002n);
    expect(q.shares).toBe(1_001n);
    expect(q.minShares).toBe(990n);
    expect(() => quoteBoingLpVaultV2Deposit({ ...s, totalLp: 0n }, 5n)).toThrow(/no liquidity/);
  });
  it('withdraw is pro-rata', () => {
    expect(quoteBoingLpVaultV2Withdraw(s, 1_000n, 0n)).toEqual({ outA: 1_000n, outB: 2_000n, minA: 1_000n, minB: 2_000n });
  });
});

describe('flows', () => {
  const cfg = { vaultHex: '0x' + 'd0'.repeat(32), poolHex: V.poolHex, tokenAHex: V.tokenAHex, tokenBHex: V.tokenBHex };
  it('deposit = exact approve A, exact approve B (spender = vault), deposit; nonces consecutive', () => {
    const [a, b, d] = buildBoingLpVaultV2DepositFlow(7n, V.senderHex, cfg, 10n, 20n, 9n);
    expect([a!.nonce, b!.nonce, d!.nonce]).toEqual([7n, 8n, 9n]);
    for (const [tx, tok, amt] of [[a, V.tokenAHex, 10n], [b, V.tokenBHex, 20n]] as const) {
      if (tx!.payload.kind !== 'contractCall') throw new Error('call');
      expect(bytesToHex(tx!.payload.contract)).toBe(tok);
      const cd = bytesToHex(tx!.payload.calldata);
      expect(cd).toContain('d0'.repeat(32));
      expect(cd.endsWith(amt.toString(16).padStart(32, '0'))).toBe(true);
    }
  });
  it('dry runs never submit', async () => {
    const submitTransaction = vi.fn();
    const client = { submitTransaction } as never;
    const key = hexToBytes(V.secretHex);
    const d = await depositBoingLpVaultV2(client, key, { ...cfg, amountA: 10n, amountB: 20n, minShares: 1n, nonce: 1n, dryRun: true });
    expect(d.txs).toHaveLength(3);
    const w = await withdrawBoingLpVaultV2(client, key, { ...cfg, shares: 5n, minA: 1n, minB: 1n, nonce: 4n, dryRun: true });
    expect(w.tx.submitted).toBe(false);
    expect(submitTransaction).not.toHaveBeenCalled();
  });
});