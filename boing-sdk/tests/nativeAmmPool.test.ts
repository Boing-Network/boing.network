import { describe, expect, it, vi } from 'vitest';
import {
  buildNativeConstantProductPoolAccessList,
  buildNativeConstantProductContractCallTx,
  mergeNativePoolAccessListWithSimulation,
  decodeBoingStorageWordU128,
  decodeNativeAmmAddLiquidityReturnLpMinted,
  decodeNativeAmmLogDataU128Triple,
  fetchNativeAmmSignerLpBalance,
  fetchNativeConstantProductPoolSnapshot,
  fetchNativeConstantProductReserves,
  fetchNativeConstantProductTotalLpSupply,
  nativeAmmLpBalanceStorageKeyHex,
  NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX,
  NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX,
  fetchNativeConstantProductSwapFeeBps,
} from '../src/nativeAmmPool.js';
import type { BoingClient } from '../src/client.js';
import type { SimulateResult } from '../src/types.js';

const SENDER = '0x' + 'ab'.repeat(32);
const POOL = '0x' + 'cd'.repeat(32);

describe('nativeAmmPool', () => {
  it('buildNativeConstantProductPoolAccessList', () => {
    const al = buildNativeConstantProductPoolAccessList(SENDER, POOL);
    expect(al.read).toEqual([SENDER.toLowerCase(), POOL.toLowerCase()]);
    expect(al.write).toEqual(al.read);
  });

  it('buildNativeConstantProductPoolAccessList merges additional token contracts (sorted, deduped)', () => {
    const tB = '0x' + '11'.repeat(32);
    const tA = '0x' + '22'.repeat(32);
    const al = buildNativeConstantProductPoolAccessList(SENDER, POOL, {
      additionalAccountsHex32: [tB, tA, tB],
    });
    expect(al.read).toEqual([
      SENDER.toLowerCase(),
      POOL.toLowerCase(),
      tB.toLowerCase(),
      tA.toLowerCase(),
    ]);
    expect(al.write).toEqual(al.read);
  });

  it('buildNativeConstantProductContractCallTx', () => {
    const tx = buildNativeConstantProductContractCallTx(SENDER, POOL, '0x' + '10'.repeat(8));
    expect(tx.type).toBe('contract_call');
    expect(tx.contract).toBe(POOL.toLowerCase());
    expect(tx.calldata).toBe('0x' + '10'.repeat(8));
    expect(tx.access_list.read.length).toBe(2);
  });

  it('mergeNativePoolAccessListWithSimulation uses optional additional accounts as base', () => {
    const tA = '0x' + '33'.repeat(32);
    const sim = {} as SimulateResult;
    const m = mergeNativePoolAccessListWithSimulation(SENDER, POOL, sim, {
      additionalAccountsHex32: [tA],
    });
    expect(m.read).toEqual([
      SENDER.toLowerCase(),
      POOL.toLowerCase(),
      tA.toLowerCase(),
    ]);
  });

  it('reserve storage keys match native_amm (byte 31 = 0x01 / 0x02)', () => {
    expect(NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX).toBe('0x' + '00'.repeat(31) + '01');
    expect(NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX).toBe('0x' + '00'.repeat(31) + '02');
  });

  it('total LP key matches native_amm (byte 31 = 0x03)', () => {
    expect(NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX).toBe('0x' + '00'.repeat(31) + '03');
  });

  it('swap fee bps key matches native_amm (byte 31 = 0x07)', () => {
    expect(NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX).toBe('0x' + '00'.repeat(31) + '07');
  });

  it('fetchNativeConstantProductSwapFeeBps', async () => {
    const getContractStorage = vi.fn(async (_pool: string, key: string) => {
      expect(key).toBe(NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX);
      return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0064' };
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const n = await fetchNativeConstantProductSwapFeeBps(client, POOL);
    expect(n).toBe(100n);
    expect(getContractStorage).toHaveBeenCalledTimes(1);
  });

  it('nativeAmmLpBalanceStorageKeyHex: zero address equals UTF-8 xor mask (padded)', () => {
    const zero = '0x' + '00'.repeat(32);
    const k = nativeAmmLpBalanceStorageKeyHex(zero);
    const label = new TextEncoder().encode('BOING_NATIVEAMM_LPRV1');
    expect(k.length).toBe(66);
    for (let i = 0; i < 32; i++) {
      const expected = i < label.length ? label[i]! : 0;
      expect(parseInt(k.slice(2 + i * 2, 2 + i * 2 + 2), 16)).toBe(expected);
    }
  });

  it('decodeNativeAmmLogDataU128Triple parses three amount words', () => {
    const w0 = '00'.repeat(32);
    const w1 = '00'.repeat(16) + '000000000000000000000000000003e8';
    const w2 = '00'.repeat(16) + '0000000000000000000000000000007b';
    const data = '0x' + w0 + w1 + w2;
    const [x, y, z3] = decodeNativeAmmLogDataU128Triple(data);
    expect(x).toBe(0n);
    expect(y).toBe(1000n);
    expect(z3).toBe(123n);
  });

  it('decodeBoingStorageWordU128 reads low 16 bytes BE', () => {
    const word =
      '0x' +
      'aa'.repeat(16) +
      'ff'.repeat(16);
    expect(decodeBoingStorageWordU128(word)).toBe(BigInt('0x' + 'ff'.repeat(16)));
    expect(decodeBoingStorageWordU128('0x')).toBe(0n);
  });

  it('decodeNativeAmmAddLiquidityReturnLpMinted parses canonical 32-byte return', () => {
    const word = '0x' + '00'.repeat(16) + '00'.repeat(14) + '162e';
    expect(decodeNativeAmmAddLiquidityReturnLpMinted(word)).toBe(5678n);
    expect(() => decodeNativeAmmAddLiquidityReturnLpMinted('0x')).toThrow(/64 hex/);
    expect(() => decodeNativeAmmAddLiquidityReturnLpMinted('0x' + '00'.repeat(63))).toThrow(/64 hex/);
    expect(() => decodeNativeAmmAddLiquidityReturnLpMinted('0x' + '01' + '00'.repeat(31))).toThrow(/high 16/);
  });

  it('fetchNativeConstantProductReserves', async () => {
    const getContractStorage = vi.fn(async (_pool: string, key: string) => {
      if (key === NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX) {
        return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0064' };
      }
      if (key === NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX) {
        return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '03e8' };
      }
      throw new Error(`unexpected key ${key}`);
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const r = await fetchNativeConstantProductReserves(client, POOL);
    expect(r.reserveA).toBe(100n);
    expect(r.reserveB).toBe(1000n);
    expect(getContractStorage).toHaveBeenCalledTimes(2);
  });

  it('fetchNativeConstantProductTotalLpSupply', async () => {
    const getContractStorage = vi.fn(async (_pool: string, key: string) => {
      if (key === NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX) {
        return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '002a' };
      }
      throw new Error(`unexpected key ${key}`);
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const n = await fetchNativeConstantProductTotalLpSupply(client, POOL);
    expect(n).toBe(42n);
    expect(getContractStorage).toHaveBeenCalledTimes(1);
  });

  it('fetchNativeAmmSignerLpBalance uses xor key', async () => {
    const key = nativeAmmLpBalanceStorageKeyHex(SENDER);
    const getContractStorage = vi.fn(async (_pool: string, k: string) => {
      expect(k).toBe(key);
      return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0005' };
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const n = await fetchNativeAmmSignerLpBalance(client, POOL, SENDER);
    expect(n).toBe(5n);
  });

  it('fetchNativeConstantProductPoolSnapshot batches 3 reads', async () => {
    const getContractStorage = vi.fn(async (_pool: string, k: string) => {
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX) {
        return { value: '0x' + '00'.repeat(32) };
      }
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX) {
        return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0002' };
      }
      if (k === NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX) {
        return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0003' };
      }
      throw new Error(`unexpected key ${k}`);
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const s = await fetchNativeConstantProductPoolSnapshot(client, POOL);
    expect(s).toEqual({ reserveA: 0n, reserveB: 2n, totalLpSupply: 3n });
    expect(getContractStorage).toHaveBeenCalledTimes(3);
  });

  it('fetchNativeConstantProductPoolSnapshot batches 4 reads with signer', async () => {
    const lpKey = nativeAmmLpBalanceStorageKeyHex(SENDER);
    const getContractStorage = vi.fn(async (_pool: string, k: string) => {
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX) return { value: '0x' + '00'.repeat(32) };
      if (k === NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX) return { value: '0x' + '00'.repeat(32) };
      if (k === NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX) return { value: '0x' + '00'.repeat(32) };
      if (k === lpKey) return { value: '0x' + '00'.repeat(16) + '00'.repeat(14) + '0007' };
      throw new Error(`unexpected key ${k}`);
    });
    const client = { getContractStorage } as unknown as BoingClient;
    const s = await fetchNativeConstantProductPoolSnapshot(client, POOL, { signerHex32: SENDER });
    expect(s.reserveA).toBe(0n);
    expect(s.signerLpBalance).toBe(7n);
    expect(getContractStorage).toHaveBeenCalledTimes(4);
  });
});
