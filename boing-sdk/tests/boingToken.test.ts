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
  approveBoingToken,
  boingTokenAllowanceStorageKeyHex,
  getBoingTokenAllowance,
  BOING_TOKEN_DEFAULT_VERSION,
} from '../src/boingToken.js';
import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX } from '../src/defaultReferenceFungibleTemplateV2BytecodeHex.js';
import { hexToBytes } from '../src/hex.js';
import { encodeReferenceMintFirstCalldata, encodeReferenceTransferCalldata, isReferenceSelfTransferCalldata } from '../src/referenceToken.js';
import type { BoingClient } from '../src/client.js';

/** Vectors produced by `boing token ... --dry-run` (crates/boing-cli) with the public test seed 0x07*32. */
const V = JSON.parse(readFileSync(new URL('./fixtures/token-cli-vectors.json', import.meta.url), 'utf8'));
const SEED = hexToBytes(V.secretSeedHex);

describe('boing token SDK helpers: byte-identical to the Rust CLI', () => {
  it('bytecode matches `boing token bytecode`', () => {
    expect(boingTokenBytecodeHex(1)).toBe(V.bytecodeHex.toLowerCase());
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
      version: 1,
      allowRetired: true,
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
    const r = await deployBoingToken(null, SEED, {
      nonce: BigInt(d.nonce),
      symbol: d.symbol,
      dryRun: true,
      version: 1,
      allowRetired: true,
    });
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
    const r = await deployBoingToken(client, SEED, {
      name: d.name,
      symbol: d.symbol,
      initialSupply: BigInt(d.initialSupply),
      mintToHex: d.mintTo,
      version: 1,
      allowRetired: true,
    });
    expect(submitTransaction).toHaveBeenNthCalledWith(1, d.signed[0]);
    expect(submitTransaction).toHaveBeenNthCalledWith(2, d.signed[1]);
    expect(r.mintTx?.submitted).toBe(true);
  });

  it('refuses retired v1 for new deploys unless allowRetired', async () => {
    await expect(
      deployBoingToken(null, SEED, { nonce: 0n, symbol: 'X', dryRun: true, version: 1 }),
    ).rejects.toThrow(/retired/);
  });
});
/** Token v2 vectors are generated by the CLI's Rust tests into pool-cli-vectors.json (`v8` block). */
const P = JSON.parse(readFileSync(new URL('./fixtures/pool-cli-vectors.json', import.meta.url), 'utf8'));

describe('boing token v2 (approve / allowance): byte-identical to the Rust CLI', () => {
  it('defaults to v2 like `boing token deploy` and matches the CLI v2 deploy', async () => {
    expect(BOING_TOKEN_DEFAULT_VERSION).toBe(2);
    expect(boingTokenBytecodeHex()).toBe(DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX);
    const r = await deployBoingToken(null, SEED, { nonce: 0n, name: 'Boing V2', symbol: 'BV2', dryRun: true });
    expect(r.deployTx.signedHex).toBe(P.v8.tokenV2Deploy.signedHex);
    expect(r.deployTx.txId).toBe(P.v8.tokenV2Deploy.txId);
  });

  it('approve matches CLI and allowance reads the v2 key', async () => {
    const a = await approveBoingToken(null, SEED, { tokenHex: P.tokenAHex, spenderHex: P.poolHex, amount: 10_000n, nonce: 4n, dryRun: true });
    expect(a.signedHex).toBe(P.v8.approveA.signedHex);
    const key = boingTokenAllowanceStorageKeyHex(P.senderHex, P.poolHex);
    expect(key).toBe(P.v8.allowanceKeyHex);
    const getContractStorage = vi.fn(async () => ({ value: `0x${'00'.repeat(31)}2a` }));
    const client = { getContractStorage } as unknown as BoingClient;
    await expect(getBoingTokenAllowance(client, P.tokenAHex, P.senderHex, P.poolHex)).resolves.toBe(42n);
    expect(getContractStorage).toHaveBeenCalledWith(P.tokenAHex, key);
  });
});

describe('self-transfer guard (v1 balance-doubling bug)', () => {
  it('transfer helpers refuse to = signer; calldata detector flags it', async () => {
    await expect(transferBoingToken(null, SEED, { nonce: 0n, tokenHex: V.transfer.token, toHex: V.deployer, amount: 5n, dryRun: true })).rejects.toThrow(/self-transfer/);
    const cd = encodeReferenceTransferCalldata(V.deployer, 5n);
    expect(isReferenceSelfTransferCalldata(V.deployer, cd)).toBe(true);
    expect(isReferenceSelfTransferCalldata(V.bob, cd)).toBe(false);
    expect(isReferenceSelfTransferCalldata(V.deployer, `0x${Buffer.from(cd).toString('hex')}`)).toBe(true);
    expect(isReferenceSelfTransferCalldata(V.deployer, encodeReferenceMintFirstCalldata(V.deployer, 5n))).toBe(false);
  });
});