import { describe, expect, it } from 'vitest';
import {
  encodeTransactionPayload,
  MAX_BATCH_CALLS,
  PayloadVariant,
  validateBatchCalls,
} from '../src/bincode.js';
import { buildBatchTransaction } from '../src/transactionBuilder.js';
import { bytesToHex } from '../src/hex.js';

const id = (b: number) => new Uint8Array(32).fill(b);
const hex = (b: number) => '0x' + b.toString(16).padStart(2, '0').repeat(32);

describe('native batch payload (TransactionPayload::Batch)', () => {
  it('matches the Rust bincode layout (crates/boing-execution/tests/batch_tx.rs)', () => {
    const enc = encodeTransactionPayload({
      kind: 'batch',
      calls: [
        { kind: 'transfer', to: id(2), amount: 7n },
        { kind: 'contractCall', contract: id(3), calldata: new Uint8Array([0xab, 0xcd]) },
      ],
    });
    const want =
      '09000000' + // Batch
      '0200000000000000' + // 2 calls
      '00000000' + '02'.repeat(32) + '07' + '00'.repeat(15) + // Transfer
      '01000000' + '03'.repeat(32) + '0200000000000000' + 'abcd'; // ContractCall
    expect(bytesToHex(enc).replace(/^0x/, '')).toBe(want);
    expect(PayloadVariant.Batch).toBe(9);
  });

  it('enforces the protocol limits', () => {
    expect(() => validateBatchCalls([])).toThrow(/no calls/);
    const many = Array.from({ length: MAX_BATCH_CALLS + 1 }, () => ({
      kind: 'transfer' as const,
      to: id(1),
      amount: 1n,
    }));
    expect(() => validateBatchCalls(many)).toThrow(/max 16/);
    expect(() =>
      validateBatchCalls([
        { kind: 'contractCall', contract: id(1), calldata: new Uint8Array(64 * 1024 + 1) },
      ]),
    ).toThrow(/calldata/);
    expect(() =>
      validateBatchCalls([
        { kind: 'transfer', to: id(1), amount: (1n << 128n) - 1n },
        { kind: 'transfer', to: id(2), amount: 1n },
      ]),
    ).toThrow(/overflow/);
  });

  it('builder fills a deduplicated access list with sender + targets', () => {
    const tx = buildBatchTransaction({
      nonce: 4n,
      senderHex: hex(0xa1),
      calls: [
        { kind: 'transfer', toHex: hex(0xb0), amount: 1n },
        { kind: 'transfer', toHex: hex(0xb0), amount: 2n },
        { kind: 'contractCall', contractHex: hex(0xc0), calldata: new Uint8Array([1]) },
      ],
    });
    expect(tx.payload.kind).toBe('batch');
    expect(tx.accessList.write.map((a) => bytesToHex(a))).toEqual([hex(0xa1), hex(0xb0), hex(0xc0)]);
    expect(tx.accessList.read).toHaveLength(3);
  });
});
