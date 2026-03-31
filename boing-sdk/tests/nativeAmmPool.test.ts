import { describe, expect, it } from 'vitest';
import {
  buildNativeConstantProductPoolAccessList,
  buildNativeConstantProductContractCallTx,
} from '../src/nativeAmmPool.js';

const SENDER = '0x' + 'ab'.repeat(32);
const POOL = '0x' + 'cd'.repeat(32);

describe('nativeAmmPool', () => {
  it('buildNativeConstantProductPoolAccessList', () => {
    const al = buildNativeConstantProductPoolAccessList(SENDER, POOL);
    expect(al.read).toEqual([SENDER.toLowerCase(), POOL.toLowerCase()]);
    expect(al.write).toEqual(al.read);
  });

  it('buildNativeConstantProductContractCallTx', () => {
    const tx = buildNativeConstantProductContractCallTx(SENDER, POOL, '0x' + '10'.repeat(8));
    expect(tx.type).toBe('contract_call');
    expect(tx.contract).toBe(POOL.toLowerCase());
    expect(tx.calldata).toBe('0x' + '10'.repeat(8));
    expect(tx.access_list.read.length).toBe(2);
  });
});
