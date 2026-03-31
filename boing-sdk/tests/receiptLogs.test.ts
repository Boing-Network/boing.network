import { describe, expect, it } from 'vitest';
import {
  filterReceiptLogsByTopic0,
  iterBlockReceiptLogs,
  logMatchesTopicFilter,
  logTopic0,
  normalizeExecutionLog,
  normalizeTopicWord,
} from '../src/receiptLogs.js';
import type { Block, ExecutionReceipt } from '../src/types.js';

const T0 = '0x' + 'ab'.repeat(32);

describe('receiptLogs', () => {
  it('normalizes topic and log', () => {
    expect(normalizeTopicWord('AB'.repeat(32))).toBe(T0);
    const log = normalizeExecutionLog({
      topics: [T0.toUpperCase()],
      data: '0x',
    });
    expect(log.topics[0]).toBe(T0);
    expect(log.data).toBe('0x');
  });

  it('topic filter and topic0', () => {
    const log = { topics: [T0], data: '0x' };
    expect(logTopic0(log)).toBe(T0);
    expect(logMatchesTopicFilter(log, [T0, null])).toBe(true);
    expect(logMatchesTopicFilter(log, ['0x' + '00'.repeat(32)])).toBe(false);
  });

  it('filterReceiptLogsByTopic0', () => {
    const receipt: ExecutionReceipt = {
      tx_id: '0x' + '01'.repeat(32),
      block_height: 1,
      tx_index: 0,
      success: true,
      gas_used: 1,
      return_data: '0x',
      logs: [
        { topics: [T0], data: '0x' },
        { topics: ['0x' + 'cd'.repeat(32)], data: '0x01' },
      ],
    };
    expect(filterReceiptLogsByTopic0(receipt, T0)).toHaveLength(1);
  });

  it('iterBlockReceiptLogs skips null receipts', () => {
    const block = {
      header: {} as Block['header'],
      transactions: [],
      receipts: [null, { tx_id: '0x' + '02'.repeat(32), block_height: 1, tx_index: 0, success: true, gas_used: 1, return_data: '0x', logs: [{ topics: [T0], data: '0x' }] }],
    } as Block;
    const refs = [...iterBlockReceiptLogs(block)];
    expect(refs).toHaveLength(1);
    expect(refs[0]!.logIndex).toBe(0);
  });
});
