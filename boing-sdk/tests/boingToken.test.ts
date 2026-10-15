import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import {
  BOING_TOKEN_PURPOSE,
  boingTokenBalanceStorageKeyHex,
  boingTokenBytecodeHex,
  buildBoingTokenDeployTransaction,
  decodeBoingTokenBalanceWord,
  deployBoingToken,
  getBoingTokenBalance,
  mintBoingToken,
  transferBoingToken,
  validateBoingTokenMetadata,
} from '../src/boingToken.js';
import { hexToBytes } from '../src/hex.js';
import type { BoingClient } from '../src/client.js';

/** Vectors produced by `boing token ... --dry-run` (crates/boing-cli) with the public test seed 0x07*32. */
const V = JSON.parse(readFileSync(new URL('./fixtures/token-cli-vectors.json', import.meta.url), 'utf8'));
const SEED = hexToBytes(V.secretSeedHex);

describe('boing token SDK helpers: byte-identical to the Rust CLI', () => {
  it('bytecode matches `boing token bytecode`', () => {
    expect(boingTokenBytecodeHex()).toBe(V.bytecodeHex.toLowerCase());
  });

  it('deploy with name/symbol/initial supply/mint-to matches CLI --dry-run', async () => {
    const d = V.deployWithMetadata;
    const r = await deployBoingToken(null, SEED, {
      nonce: BigInt(d.nonce),
      name: d.name,
      symbol: d.symbol,
      initialSupply: BigInt(d.initialSupply),
      mintToHex: d.mintTo,
      dryRun: true,
    });
    expect(r.deployerHex).toBe(V.deployer);
    expect(r.tokenHex).toBe(d.token);
    expect(r.deployTx.signedHex).toBe(d.signed[0]);
    expect(r.deployTx.txId).toBe(d.txIds[0]);
    expect(r.mintTx?.signedHex).toBe(d.signed[1]);
    expect(r.mintTx?.txId).toBe(d.txIds[1]);
    expect(r.deployTx.submitted).toBe(false);
  });

  it('symbol-only deploy without supply matches CLI', async () => {
    const d = V.deploySymbolOnly;
    const r = await deployBoingToken(null, SEED, { nonce: BigInt(d.nonce), symbol: d.symbol, dryRun: true });
    expect(r.tokenHex).toBe(d.token);
    expect(r.deployTx.signedHex).toBe(d.signed[0]);
    expect(r.mintTx).toBeNull();
  });

  it('transfer matches CLI', async () => {
    const t = V.transfer;
    const r = await transferBoingToken(null, SEED, { nonce: BigInt(t.nonce), tokenHex: t.token, toHex: t.to, amount: BigInt(t.amount), dryRun: true });
    expect(r.signedHex).toBe(t.signed);
    expect(r.txId).toBe(t.txId);
  });

  it('mint (default recipient = signer) matches CLI', async () => {
    const m = V.mintToSelf;
    const r = await mintBoingToken(null, SEED, { nonce: BigInt(m.nonce), tokenHex: m.token, amount: BigInt(m.amount), dryRun: true });
    expect(r.signedHex).toBe(m.signed);
    expect(r.txId).toBe(m.txId);
  });
});

describe('boing token SDK helpers: behaviour', () => {
  it('deploy tx uses purpose token and sender-only access list', () => {
    const tx = buildBoingTokenDeployTransaction({ nonce: 1n, senderHex: V.deployer, symbol: 'X' });
    expect(tx.payload.kind).toBe('contractDeployWithPurposeAndMetadata');
    if (tx.payload.kind === 'contractDeployWithPurposeAndMetadata') expect(tx.payload.purposeCategory).toBe(BOING_TOKEN_PURPOSE);
    expect(tx.accessList.read).toHaveLength(1);
  });

  it('validates metadata, amounts and balance words like the CLI', async () => {
    expect(() => validateBoingTokenMetadata(null, 'X'.repeat(33))).toThrow(/symbol/);
    expect(() => validateBoingTokenMetadata('n'.repeat(257), null)).toThrow(/name/);
    await expect(transferBoingToken(null, SEED, { nonce: 0n, tokenHex: V.transfer.token, toHex: V.bob, amount: 0n, dryRun: true })).rejects.toThrow(/> 0/);
    expect(decodeBoingTokenBalanceWord(`0x${'00'.repeat(16)}${(42n).toString(16).padStart(32, '0')}`)).toBe(42n);
    expect(() => decodeBoingTokenBalanceWord(`0x${'ff'.repeat(32)}`)).toThrow(/u128/);
    await expect(deployBoingToken(null, SEED, { dryRun: true })).rejects.toThrow(/nonce/);
  });

  it('balance storage key is holder ^ BOING_REFFUNGIBLE_BAL1 and balance reads it', async () => {
    const holder = `0x${'00'.repeat(32)}`;
    const key = boingTokenBalanceStorageKeyHex(holder);
    expect(key).toBe(`0x${Buffer.from('BOING_REFFUNGIBLE_BAL1').toString('hex')}${'00'.repeat(10)}`);
    const getContractStorage = vi.fn(async () => ({ value: `0x${'00'.repeat(31)}07` }));
    const client = { getContractStorage } as unknown as BoingClient;
    await expect(getBoingTokenBalance(client, V.transfer.token, holder)).resolves.toBe(7n);
    expect(getContractStorage).toHaveBeenCalledWith(V.transfer.token, key);
  });

  it('submits through the client and fetches the nonce when not given', async () => {
    const submitTransaction = vi.fn(async () => ({}));
    const getAccount = vi.fn(async () => ({ nonce: 3, balance: '0', stake: '0' }));
    const client = { submitTransaction, getAccount } as unknown as BoingClient;
    const d = V.deployWithMetadata;
    const r = await deployBoingToken(client, SEED, { name: d.name, symbol: d.symbol, initialSupply: BigInt(d.initialSupply), mintToHex: d.mintTo });
    expect(submitTransaction).toHaveBeenNthCalledWith(1, d.signed[0]);
    expect(submitTransaction).toHaveBeenNthCalledWith(2, d.signed[1]);
    expect(r.mintTx?.submitted).toBe(true);
  });
});