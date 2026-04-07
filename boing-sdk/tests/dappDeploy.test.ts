import { describe, expect, it, vi } from 'vitest';
import { buildContractDeployMetaTx } from '../src/canonicalDeployArtifacts.js';
import {
  BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX,
  buildAndPreflightReferenceFungibleDeploy,
  buildBoingIntegrationDeployMetaTx,
  defaultPurposeCategoryForBoingDeployKind,
  describeContractDeployMetaQaResponse,
  preflightBoingIntegrationDeploy,
  preflightContractDeployMetaQa,
  preflightContractDeployMetaWithUi,
} from '../src/dappDeploy.js';
import type { BoingClient } from '../src/client.js';
import type { QaCheckResponse } from '../src/types.js';

describe('dappDeploy', () => {
  it('BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX is 32 bytes', () => {
    expect(BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX).toBe(`0x${'00'.repeat(32)}`);
  });

  it('preflightContractDeployMetaQa uses description_hash when set', async () => {
    const qaCheck = vi.fn().mockResolvedValue({ result: 'allow' } satisfies QaCheckResponse);
    const client = { qaCheck } as unknown as BoingClient;
    const tx = buildContractDeployMetaTx({
      bytecodeHex: '0xab',
      assetName: 'N',
      assetSymbol: 's',
      descriptionHashHex: '0x' + 'cc'.repeat(32),
    });
    const r = await preflightContractDeployMetaQa(client, tx);
    expect(r.result).toBe('allow');
    expect(qaCheck).toHaveBeenCalledWith(
      tx.bytecode,
      tx.purpose_category,
      tx.description_hash,
      tx.asset_name,
      tx.asset_symbol,
    );
  });

  it('preflightContractDeployMetaQa uses placeholder when description_hash omitted', async () => {
    const qaCheck = vi.fn().mockResolvedValue({ result: 'unsure' } satisfies QaCheckResponse);
    const client = { qaCheck } as unknown as BoingClient;
    const tx = buildContractDeployMetaTx({
      bytecodeHex: '0xab',
      assetName: 'N',
      assetSymbol: 's',
    });
    const r = await preflightContractDeployMetaQa(client, tx);
    expect(r.result).toBe('unsure');
    expect(qaCheck).toHaveBeenCalledWith(
      tx.bytecode,
      tx.purpose_category,
      BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX,
      tx.asset_name,
      tx.asset_symbol,
    );
  });

  it('describeContractDeployMetaQaResponse maps reject with rule and message', () => {
    const ui = describeContractDeployMetaQaResponse({
      result: 'reject',
      rule_id: 'MAX_BYTECODE_SIZE',
      message: 'too large',
    });
    expect(ui.readyToSign).toBe(false);
    expect(ui.tone).toBe('destructive');
    expect(ui.detail).toContain('MAX_BYTECODE_SIZE');
    expect(ui.detail).toContain('too large');
  });

  it('preflightContractDeployMetaWithUi returns matching qa and ui', async () => {
    const qaCheck = vi.fn().mockResolvedValue({ result: 'allow' } satisfies QaCheckResponse);
    const client = { qaCheck } as unknown as BoingClient;
    const tx = buildContractDeployMetaTx({
      bytecodeHex: '0xab',
      assetName: 'N',
      assetSymbol: 's',
    });
    const r = await preflightContractDeployMetaWithUi(client, tx);
    expect(r.qa.result).toBe('allow');
    expect(r.ui.result).toBe('allow');
    expect(r.ui.readyToSign).toBe(true);
  });

  it('buildAndPreflightReferenceFungibleDeploy builds tx and runs qa', async () => {
    const qaCheck = vi.fn().mockResolvedValue({ result: 'allow' } satisfies QaCheckResponse);
    const client = { qaCheck } as unknown as BoingClient;
    const r = await buildAndPreflightReferenceFungibleDeploy(client, {
      assetName: 'Tok',
      assetSymbol: 'TK',
    });
    expect(r.tx.type).toBe('contract_deploy_meta');
    expect(r.tx.asset_name).toBe('Tok');
    expect(r.tx.asset_symbol).toBe('TK');
    expect(r.qa.result).toBe('allow');
    expect(qaCheck).toHaveBeenCalledTimes(1);
  });

  it('defaultPurposeCategoryForBoingDeployKind maps pool to dapp', () => {
    expect(defaultPurposeCategoryForBoingDeployKind('token')).toBe('token');
    expect(defaultPurposeCategoryForBoingDeployKind('nft')).toBe('nft');
    expect(defaultPurposeCategoryForBoingDeployKind('liquidity_pool')).toBe('dapp');
  });

  it('buildBoingIntegrationDeployMetaTx sets purpose for liquidity_pool', () => {
    const tx = buildBoingIntegrationDeployMetaTx({
      kind: 'liquidity_pool',
      bytecodeHexOverride: '0xab',
    });
    expect(tx.purpose_category).toBe('dapp');
    expect(tx.asset_name).toBe('Native CP Pool');
    expect(tx.asset_symbol).toBe('POOL');
  });

  it('preflightBoingIntegrationDeploy runs qa with pool bytecode and dapp category', async () => {
    const qaCheck = vi.fn().mockResolvedValue({ result: 'allow' } satisfies QaCheckResponse);
    const client = { qaCheck } as unknown as BoingClient;
    const r = await preflightBoingIntegrationDeploy(client, {
      kind: 'liquidity_pool',
      bytecodeHexOverride: '0xcd',
      poolLabel: 'My Pool',
      poolSymbol: 'mpl',
    });
    expect(r.tx.purpose_category).toBe('dapp');
    expect(r.tx.asset_name).toBe('My Pool');
    expect(r.tx.asset_symbol).toBe('MPL');
    expect(qaCheck).toHaveBeenCalledWith(
      r.tx.bytecode,
      'dapp',
      BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX,
      'My Pool',
      'MPL',
    );
  });
});
