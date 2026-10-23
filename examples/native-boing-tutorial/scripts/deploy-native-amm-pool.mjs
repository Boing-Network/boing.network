#!/usr/bin/env node
/**
 * Deploy native constant-product pool bytecode to a Boing RPC (public testnet by default).
 * Uses CREATE2 + NATIVE_CP_POOL_CREATE2_SALT_V1 so the pool address is predictable.
 *
 * Prerequisites:
 *   - Funded deployer: BOING_SECRET_HEX = 0x + 64 hex (32-byte Ed25519 seed; NEVER commit or paste in chat).
 *   - Bytecode from: cargo run -p boing-execution --example dump_native_amm_pool
 *
 * Env:
 *   BOING_RPC_URL              — default https://testnet-rpc.boing.network
 *   BOING_SECRET_HEX           — required
 *   BOING_NATIVE_AMM_BYTECODE_FILE — path to hex file; one line, OR two lines from dump (see VARIANT)
 *   BOING_NATIVE_AMM_BYTECODE_HEX  — or inline 0x... hex (single line)
 *   BOING_NATIVE_AMM_VARIANT     — v1 (default) | v2 — which bytecode line when file has 2 lines; also picks CREATE2 salt
 *   BOING_USE_CREATE2          — default 1 (set 0 for nonce-derived deploy)
 *   BOING_PURPOSE              — default dapp (native AMM pool QA)
 *   BOING_EXPECT_SENDER_HEX    — optional; if set, must match pubkey from secret (sanity check)
 */
import {
  BoingRpcError,
  createClient,
  explainBoingRpcError,
  fetchNextNonce,
  hexToBytes,
  NATIVE_CP_POOL_CREATE2_SALT_V1,
  NATIVE_CP_POOL_CREATE2_SALT_V2,
  predictNativeCpPoolCreate2Address,
  predictNativeCpPoolV2Create2Address,
  predictNonceDerivedContractAddress,
  senderHexFromSecretKey,
  submitDeployWithPurposeFlow,
  validateHex32,
} from '@boing-network/boing-sdk';
import { readFileSync } from 'node:fs';

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const secretHex = process.env.BOING_SECRET_HEX;
const bytecodeFile = process.env.BOING_NATIVE_AMM_BYTECODE_FILE;
const bytecodeHexEnv = process.env.BOING_NATIVE_AMM_BYTECODE_HEX;
const useCreate2 = process.env.BOING_USE_CREATE2 !== '0' && process.env.BOING_USE_CREATE2 !== 'false';
const purposeCategory = process.env.BOING_PURPOSE ?? 'dapp';
const expectSender = process.env.BOING_EXPECT_SENDER_HEX?.trim();
const variant = (process.env.BOING_NATIVE_AMM_VARIANT ?? 'v1').toLowerCase();
const isV2 = variant === 'v2';
if (variant !== 'v1' && !isV2) {
  console.error('BOING_NATIVE_AMM_VARIANT must be v1 or v2');
  process.exit(1);
}

if (!secretHex) {
  console.error('Set BOING_SECRET_HEX (32-byte Ed25519 seed as 0x + 64 hex). Run locally only; never share it.');
  process.exit(1);
}

if (!bytecodeFile && !bytecodeHexEnv) {
  console.error(
    'Set BOING_NATIVE_AMM_BYTECODE_FILE (path to hex from dump_native_amm_pool) or BOING_NATIVE_AMM_BYTECODE_HEX.'
  );
  console.error('  Example: cargo run -p boing-execution --example dump_native_amm_pool > pool.hex');
  process.exit(1);
}

function pickBytecodeLine(text) {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !l.startsWith('//'));
  if (lines.length === 0) {
    throw new Error('Bytecode file is empty');
  }
  if (lines.length === 1) {
    if (isV2) {
      throw new Error(
        'BOING_NATIVE_AMM_VARIANT=v2 requires v2 bytecode (second line of dump_native_amm_pool stdout, or a one-line file containing only v2 hex)',
      );
    }
    return lines[0];
  }
  const idx = isV2 ? 1 : 0;
  if (lines.length <= idx) {
    throw new Error(
      `BOING_NATIVE_AMM_VARIANT=${variant} needs bytecode line ${idx + 1}, but only ${lines.length} line(s) found (use 2-line dump from dump_native_amm_pool)`,
    );
  }
  return lines[idx];
}

function loadBytecodeHex() {
  if (bytecodeHexEnv) {
    return pickBytecodeLine(bytecodeHexEnv);
  }
  return pickBytecodeLine(readFileSync(bytecodeFile, 'utf8'));
}

async function main() {
  const secret = hexToBytes(secretHex);
  const rawHex = loadBytecodeHex();
  const bytecode = hexToBytes(rawHex.startsWith('0x') ? rawHex : `0x${rawHex}`);

  const client = createClient(rpc);
  try {
    await client.chainHeight();
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'rpc_preflight',
          rpc,
          error: explainBoingRpcError(e),
          hint:
            'Fix RPC connectivity first (e.g. HTTP 530 → Cloudflare tunnel origin). See docs/RUNBOOK.md § 8.3 or run: npm run check-testnet-rpc',
        },
        null,
        2
      )
    );
    process.exit(1);
  }

  const senderHex = await senderHexFromSecretKey(secret);

  if (expectSender) {
    const want = validateHex32(expectSender.startsWith('0x') ? expectSender : `0x${expectSender}`);
    if (want !== senderHex) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: 'BOING_EXPECT_SENDER_HEX does not match secret key',
            expectSenderHex: want,
            derivedSenderHex: senderHex,
          },
          null,
          2
        )
      );
      process.exit(1);
    }
  }

  const create2Salt = useCreate2 ? (isV2 ? NATIVE_CP_POOL_CREATE2_SALT_V2 : NATIVE_CP_POOL_CREATE2_SALT_V1) : null;
  const deployNonce = await fetchNextNonce(client, senderHex);
  const predictedPool = useCreate2
    ? isV2
      ? predictNativeCpPoolV2Create2Address(senderHex, bytecode)
      : predictNativeCpPoolCreate2Address(senderHex, bytecode)
    : predictNonceDerivedContractAddress(senderHex, deployNonce);

  const out = await submitDeployWithPurposeFlow({
    client,
    secretKey32: secret,
    senderHex,
    bytecode,
    purposeCategory,
    create2Salt,
  });

  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        senderHex,
        variant,
        purposeCategory,
        create2: useCreate2,
        predictedPoolHex: predictedPool,
        tx_hash: out.tx_hash,
        simulationAttempts: out.attempts,
        note: useCreate2
          ? 'After inclusion, pool should be at predictedPoolHex; verify with boing_getContractStorage (reserve A key).'
          : 'Nonce-derived: predictedPoolHex = BLAKE3(sender || nonce_le) for the deploy tx nonce (see predictNonceDerivedContractAddress).',
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  const msg = explainBoingRpcError(e);
  console.error(msg);
  if (/account not found/i.test(msg)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'Deploy simulation needs your sender account in chain state (not just boing_getAccount defaults).',
          hint: 'Run: npm run fund-deployer-from-env — or BOING_AUTO_FAUCET_REQUEST=1 with deploy-native-dex-full-stack. Or use https://boing.network/faucet for senderHex from your secret.',
        },
        null,
        2
      )
    );
  }
  if (/deployment address already has an account or code/i.test(msg)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'CREATE2 target address is already occupied (common on public testnet).',
          hint: 'Retry with BOING_USE_CREATE2=0 for a fresh nonce-derived pool id, or reuse the existing pool at predictedPoolHex if bytecode matches.',
        },
        null,
        2
      )
    );
  }
  if (
    e instanceof BoingRpcError &&
    e.qaData?.rule_id === 'INVALID_OPCODE' &&
    /\boffset 67\b/i.test(e.qaData.message ?? '')
  ) {
    console.error(
      [
        'Hint: Opcode at bytecode offset 67 is 0x14 (EQ), used for native AMM calldata dispatch.',
        'Your RPC node QA whitelist is likely older than this repo: current main allows EQ and other comparators;',
        'stale testnets only allowed a minimal opcode set and reject this bytecode.',
        'Use BOING_RPC_URL pointing at a boing-node built from current main, or ask operators to upgrade testnet.',
      ].join(' ')
    );
  }
  process.exit(1);
});
