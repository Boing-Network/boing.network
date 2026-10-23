#!/usr/bin/env node
/**
 * One-shot operator flow: optional bytecode dump → deploy native CP pool → deploy DEX pair directory
 * → optional register_pair (synthetic token ids for devnet smoke, or your own).
 *
 * Prerequisites:
 *   - Monorepo: `cargo` + built `boing-execution` examples
 *   - `cd ../../boing-sdk && npm run build` and `npm install` in this tutorial package
 *   - Funded signer: BOING_SECRET_HEX (never commit or paste)
 *
 * Env:
 *   BOING_SECRET_HEX              — required
 *   BOING_RPC_URL                 — forwarded (default from child scripts = public testnet)
 *   BOING_SKIP_DUMP               — set `1` to reuse existing artifacts/pool-lines.hex + native-dex-factory.hex
 *   BOING_NATIVE_AMM_VARIANT      — v1 (default) or v2
 *   BOING_BOOTSTRAP_REGISTER_PAIR — set `1` to submit register_pair after factory deploy
 *   BOING_DEX_TOKEN_A_HEX         — optional with register; default when unset = 32× 0xaa (demo only)
 *   BOING_DEX_TOKEN_B_HEX         — optional with register; default when unset = 32× 0xbb (demo only)
 *   BOING_BOOTSTRAP_NO_AUTO_NONCE — set `1` to disable auto-retry when CREATE2 hits “address already has code”
 *   BOING_BOOTSTRAP_POOL_COMMIT_WAIT_MS — max wait (default 120000) for `boing_getAccount.nonce` to advance after pool deploy (committed state)
 *
 * Forwarded to deploy scripts: BOING_USE_CREATE2, BOING_PURPOSE, BOING_EXPECT_SENDER_HEX, etc.
 *
 * **CREATE2 collision:** On public testnet the canonical CREATE2 pool/factory slots may already be taken.
 * By default this script retries each failed deploy with **`BOING_USE_CREATE2=0`** so you get fresh nonce-derived ids.
 */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createClient, hexToBytes, senderHexFromSecretKey } from '@boing-network/boing-sdk';
import { parseScriptStdoutJson } from './tutorial-script-json.mjs';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tutorialRoot = path.resolve(scriptDir, '..');
const poolFile = path.join(tutorialRoot, 'artifacts', 'pool-lines.hex');
const factoryFile = path.join(tutorialRoot, 'artifacts', 'native-dex-factory.hex');

const secretHex = process.env.BOING_SECRET_HEX;
if (!secretHex) {
  console.error('Set BOING_SECRET_HEX (0x + 64 hex). Run locally only; never share it.');
  process.exit(1);
}

const noAutoNonce =
  process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === '1' || process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === 'true';
const userCreate2Off = process.env.BOING_USE_CREATE2 === '0' || process.env.BOING_USE_CREATE2 === 'false';
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function parseDeployJson(stdout) {
  return parseScriptStdoutJson(stdout);
}

function runNodeScript(relScript, extraEnv) {
  const r = spawnSync(process.execPath, [path.join(scriptDir, relScript)], {
    cwd: tutorialRoot,
    encoding: 'utf8',
    env: { ...process.env, ...extraEnv },
    maxBuffer: 16 * 1024 * 1024,
  });
  if (r.error) throw r.error;
  return {
    ok: r.status === 0,
    status: r.status ?? 1,
    stdout: r.stdout ?? '',
    stderr: r.stderr ?? '',
  };
}

function isCreate2AddressInUse(res) {
  return /deployment address already has an account or code/i.test(`${res.stdout}\n${res.stderr}`);
}

async function waitCommittedNonceAdvanced(client, senderHex, nonceBefore, timeoutMs) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const acc = await client.getAccount(senderHex);
    if (BigInt(acc.nonce) > nonceBefore) return;
    await sleep(400);
  }
  throw new Error(
    `Timeout ${timeoutMs}ms: sender nonce did not advance on ${rpc} (pool deploy may not be committed yet).`
  );
}

async function main() {
  const secret = hexToBytes(secretHex);
  const senderHexForNonce = await senderHexFromSecretKey(secret);
  const client = createClient(rpc);
  const nonceBeforePool = BigInt((await client.getAccount(senderHexForNonce)).nonce);

  const skipDump = process.env.BOING_SKIP_DUMP === '1' || process.env.BOING_SKIP_DUMP === 'true';
  if (!skipDump) {
    const d = runNodeScript('dump-native-bytecodes.mjs', {});
    if (!d.ok) {
      console.error(d.stderr || d.stdout);
      process.exit(d.status);
    }
  } else {
    if (!existsSync(poolFile) || !existsSync(factoryFile)) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: 'BOING_SKIP_DUMP set but artifacts missing',
            need: [poolFile, factoryFile],
            hint: 'Run npm run dump-native-bytecodes or unset BOING_SKIP_DUMP',
          },
          null,
          2
        )
      );
      process.exit(1);
    }
  }

  const variant = (process.env.BOING_NATIVE_AMM_VARIANT ?? 'v1').toLowerCase();
  const basePoolEnv = {
    BOING_SECRET_HEX: secretHex,
    BOING_NATIVE_AMM_BYTECODE_FILE: poolFile,
    BOING_NATIVE_AMM_VARIANT: variant,
  };

  let poolOut = runNodeScript('deploy-native-amm-pool.mjs', basePoolEnv);
  let poolRetriedWithNonce = false;
  if (!poolOut.ok && !userCreate2Off && !noAutoNonce && isCreate2AddressInUse(poolOut)) {
    console.warn(
      '[bootstrap] Pool CREATE2 target is already occupied — retrying with BOING_USE_CREATE2=0 (nonce-derived pool id).'
    );
    poolOut = runNodeScript('deploy-native-amm-pool.mjs', { ...basePoolEnv, BOING_USE_CREATE2: '0' });
    poolRetriedWithNonce = true;
  }
  if (!poolOut.ok) {
    console.error(poolOut.stderr || poolOut.stdout);
    process.exit(poolOut.status ?? 1);
  }

  let poolJson;
  try {
    poolJson = parseDeployJson(poolOut.stdout);
  } catch (e) {
    console.error(poolOut.stdout);
    throw e;
  }

  if (!poolJson.ok) {
    console.error(JSON.stringify({ ok: false, phase: 'deploy_pool', poolJson }, null, 2));
    process.exit(1);
  }

  const commitWaitMs = Number(process.env.BOING_BOOTSTRAP_POOL_COMMIT_WAIT_MS ?? 120_000);
  console.warn(
    '[bootstrap] Waiting for pool deploy to commit (on-chain sender nonce must advance before factory deploy)…'
  );
  try {
    await waitCommittedNonceAdvanced(client, senderHexForNonce, nonceBeforePool, commitWaitMs);
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'wait_pool_commit',
          error: String(e?.message ?? e),
          hint: 'Produce blocks on your node or wait for testnet inclusion; then re-run from deploy-native-dex-directory with the same pool id.',
        },
        null,
        2
      )
    );
    process.exit(1);
  }

  const wantRegister =
    process.env.BOING_BOOTSTRAP_REGISTER_PAIR === '1' || process.env.BOING_BOOTSTRAP_REGISTER_PAIR === 'true';
  if (wantRegister && !poolJson.predictedPoolHex) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'register_pair',
          error:
            'predictedPoolHex missing; rebuild boing-sdk so nonce deploy reports predictedPoolHex, or set BOING_USE_CREATE2=1 with a fresh chain.',
          poolJson,
        },
        null,
        2
      )
    );
    process.exit(1);
  }
  const synthA = `0x${'aa'.repeat(32)}`;
  const synthB = `0x${'bb'.repeat(32)}`;
  const tokenA = process.env.BOING_DEX_TOKEN_A_HEX?.trim() ?? (wantRegister ? synthA : undefined);
  const tokenB = process.env.BOING_DEX_TOKEN_B_HEX?.trim() ?? (wantRegister ? synthB : undefined);

  if (wantRegister && (!tokenA || !tokenB)) {
    console.error('Internal error: register requested but token ids missing');
    process.exit(1);
  }

  if (wantRegister && !process.env.BOING_DEX_TOKEN_A_HEX && !process.env.BOING_DEX_TOKEN_B_HEX) {
    console.warn(
      'BOING_BOOTSTRAP_REGISTER_PAIR: using synthetic token ids 0xaa… and 0xbb… (devnet smoke only). Set BOING_DEX_TOKEN_A_HEX / BOING_DEX_TOKEN_B_HEX for real pairs.'
    );
  }

  /** @type {Record<string, string>} */
  const dexEnv = {
    BOING_SECRET_HEX: secretHex,
    BOING_DEX_FACTORY_BYTECODE_FILE: factoryFile,
  };
  if (wantRegister && tokenA && tokenB) {
    dexEnv.BOING_DEX_POOL_HEX = poolJson.predictedPoolHex;
    dexEnv.BOING_DEX_TOKEN_A_HEX = tokenA;
    dexEnv.BOING_DEX_TOKEN_B_HEX = tokenB;
  }

  let dexOut = runNodeScript('deploy-native-dex-directory.mjs', dexEnv);
  let factoryRetriedWithNonce = false;
  if (!dexOut.ok && !userCreate2Off && !noAutoNonce && isCreate2AddressInUse(dexOut)) {
    console.warn(
      '[bootstrap] Factory CREATE2 target is already occupied — retrying with BOING_USE_CREATE2=0 (nonce-derived directory id).'
    );
    dexOut = runNodeScript('deploy-native-dex-directory.mjs', { ...dexEnv, BOING_USE_CREATE2: '0' });
    factoryRetriedWithNonce = true;
  }
  if (!dexOut.ok) {
    console.error(dexOut.stderr || dexOut.stdout);
    process.exit(dexOut.status ?? 1);
  }

  let dexJson;
  try {
    dexJson = parseDeployJson(dexOut.stdout);
  } catch (e) {
    console.error(dexOut.stdout);
    throw e;
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        bootstrap: {
          poolRetriedWithNonce,
          factoryRetriedWithNonce,
          autoNonceOnCollisionDisabled: noAutoNonce,
        },
        pool: poolJson,
        dexDirectory: dexJson,
        registerPairSubmitted: Boolean(dexJson.register_tx_hash),
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
