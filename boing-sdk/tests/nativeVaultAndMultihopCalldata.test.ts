import { describe, expect, it } from 'vitest';
import {
  buildNativeAmmLpVaultConfigureAccessList,
  buildNativeAmmLpVaultDepositAddAccessList,
  encodeNativeAmmLpVaultConfigureCalldata,
  encodeNativeAmmLpVaultDepositAddCalldata,
} from '../src/nativeAmmLpVault.js';
import { encodeNativeAmmAddLiquidityCalldata } from '../src/nativeAmm.js';
import {
  encodeNativeDexSwap3RouterCalldata128,
  encodeNativeDexSwap4RouterCalldata160,
} from '../src/nativeDexSwap2Router.js';
import {
  buildLpShareTokenAccessList,
  buildLpShareTokenContractCallTx,
  encodeLpShareMintCalldata,
  encodeLpShareSetMinterOnceCalldata,
} from '../src/nativeLpShareToken.js';

describe('native multihop / LP vault / share calldata', () => {
  const p = '0x' + '01'.repeat(32);
  const inner = encodeNativeAmmAddLiquidityCalldata(10n, 20n, 0n);

  it('encodeNativeDexSwap3RouterCalldata128 length is 512', () => {
    const cd = encodeNativeDexSwap3RouterCalldata128(p, inner, p, inner, p, inner);
    expect(cd.length).toBe(512);
    expect(cd[31]).toBe(0xe7);
  });

  it('encodeNativeDexSwap4RouterCalldata160 length is 800', () => {
    const inner160 = new Uint8Array(160);
    inner160[31] = 0x15;
    const cd = encodeNativeDexSwap4RouterCalldata160(p, inner160, p, inner160, p, inner160, p, inner160);
    expect(cd.length).toBe(800);
    expect(cd[31]).toBe(0xea);
  });

  it('LP vault configure / deposit_add lengths', () => {
    expect(encodeNativeAmmLpVaultConfigureCalldata(p, p).length).toBe(96);
    expect(encodeNativeAmmLpVaultDepositAddCalldata(inner, 0n).length).toBe(192);
    expect(encodeNativeAmmLpVaultDepositAddCalldata(inner, 0n)[31]).toBe(0xc1);
  });

  it('LP share encoders', () => {
    expect(encodeLpShareSetMinterOnceCalldata(p).length).toBe(64);
    expect(encodeLpShareMintCalldata(p, 1n).length).toBe(96);
  });

  it('LP share access list + contract_call tx', () => {
    const s = '0x' + 'aa'.repeat(32);
    const c = '0x' + 'bb'.repeat(32);
    const al = buildLpShareTokenAccessList(s, c);
    expect(al.read).toEqual([s, c]);
    const tx = buildLpShareTokenContractCallTx(s, c, '0x' + '00'.repeat(64));
    expect(tx.contract).toBe(c);
    expect(tx.access_list).toEqual(al);
  });

  it('LP vault access lists are sorted unique hex', () => {
    const s = '0x' + 'aa'.repeat(32);
    const v = '0x' + 'bb'.repeat(32);
    const pool = '0x' + 'cc'.repeat(32);
    const share = '0x' + 'dd'.repeat(32);
    const cfg = buildNativeAmmLpVaultConfigureAccessList(s, v);
    expect(cfg.read).toEqual([s, v]);
    const dep = buildNativeAmmLpVaultDepositAddAccessList(s, v, pool, share);
    expect(dep.read).toEqual([s, v, pool, share]);
    const depDup = buildNativeAmmLpVaultDepositAddAccessList(s, v, pool, pool);
    expect(depDup.read).toEqual([s, v, pool]);
  });
});
