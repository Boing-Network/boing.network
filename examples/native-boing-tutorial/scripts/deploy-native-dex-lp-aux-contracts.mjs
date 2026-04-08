#!/usr/bin/env node
/**
 * Deploy native AMM LP vault + LP share token at canonical CREATE2 addresses (same deployer + salts as
 * [scripts/canonical-testnet-dex-predicted.json](../../../../scripts/canonical-testnet-dex-predicted.json)).
 *
 * Run after pool + directory (+ optional router deploys). See [NATIVE-AMM-LP-VAULT.md](../../../docs/NATIVE-AMM-LP-VAULT.md),
 * [NATIVE-LP-SHARE-TOKEN.md](../../../docs/NATIVE-LP-SHARE-TOKEN.md).
 *
 * Prerequisites:
 *   - `npm run dump-native-bytecodes` (or `BOING_SKIP_DUMP=1` with `artifacts/native-amm-lp-vault.hex` + `native-lp-share-token.hex`)
 *   - `boing-sdk` built; `BOING_SECRET_HEX`
 *
 * Env:
 *   BOING_SECRET_HEX, BOING_RPC_URL — required / forwarded
 *   BOING_SKIP_DUMP — `1` to skip dump-native-bytecodes.mjs first
 *   BOING_LP_AUX_SKIP_VAULT — `1` to skip LP vault deploy
 *   BOING_LP_AUX_SKIP_SHARE — `1` to skip LP share token deploy
 *   BOING_BOOTSTRAP_NO_AUTO_NONCE — `1` to disable CREATE2-collision → nonce retry
 *   BOING_AUX_COMMIT_WAIT_MS / BOING_LP_AUX_COMMIT_WAIT_MS — nonce wait after each tx (default 120000; LP prefers BOING_LP_AUX if set)
 */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createClient, explainBoingRpcError, hexToBytes, senderHexFromSecretKey, validateHex32 } from 'boing-sdk';
import {
  ScheduledExitError,
  exitAfterLog,
  getAccountScheduled,
  scheduleExit,
} from './tutorial-deploy-scheduled-exit.mjs';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tutorialRoot = path.resolve(scriptDir, '..');
const artifactsDir = path.join(tutorialRoot, 'artifacts');

const secretHex = process.env.BOING_SECRET_HEX;
if (!secretHex?.trim()) {
  console.error('Set BOING_SECRET_HEX (0x + 64 hex).');
  scheduleExit(1);
} else {
  runMain();
}

function runMain() {
  main().catch((e) => {
    if (e instanceof ScheduledExitError) return;
    console.error(explainBoingRpcError(e));
    scheduleExit(1);
  });
}

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const noAutoNonce =
  process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === '1' || process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === 'true';
const userCreate2Off = process.env.BOING_USE_CREATE2 === '0' || process.env.BOING_USE_CREATE2 === 'false';
const commitWaitMs = Number(
  process.env.BOING_LP_AUX_COMMIT_WAIT_MS ?? process.env.BOING_AUX_COMMIT_WAIT_MS ?? 120_000
);

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function runNodeScript(relScript, args, extraEnv) {
  const r = spawnSync(process.execPath, [path.join(scriptDir, relScript), ...(args ?? [])], {
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

function isCreate2Collision(res) {
  return /deployment address already has an account or code/i.test(`${res.stdout}\n${res.stderr}`);
}

function parseDeployJson(stdout) {
  const t = stdout.trim();
  const lastBrace = t.lastIndexOf('{');
  if (lastBrace < 0) throw new Error('No JSON object in script stdout');
  return JSON.parse(t.slice(lastBrace));
}

async function waitNonceAfter(client, senderHex, nonceBefore, label) {
  const t0 = Date.now();
  while (Date.now() - t0 < commitWaitMs) {
    const acc = await getAccountScheduled(client, rpc, senderHex);
    if (BigInt(acc.nonce) > nonceBefore) return;
    await sleep(400);
  }
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: `Timeout waiting for nonce to advance after ${label}`,
        phase: 'wait_nonce',
        hint: 'Ensure blocks are produced; increase BOING_LP_AUX_COMMIT_WAIT_MS or BOING_AUX_COMMIT_WAIT_MS',
      },
      null,
      2
    )
  );
  exitAfterLog(1);
}

function saltForKey(saltKey) {
  const r = runNodeScript('print-native-dex-deploy-salts.mjs', [saltKey], {});
  if (!r.ok) {
    console.error(r.stderr || r.stdout);
    exitAfterLog(r.status ?? 1);
  }
  const line = r.stdout
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => /^0x[0-9a-f]{64}$/i.test(l));
  if (!line) {
    console.error(JSON.stringify({ ok: false, error: 'bad_salt_output', saltKey, stdout: r.stdout }, null, 2));
    exitAfterLog(1);
  }
  return line;
}

async function main() {
  const skipDump = process.env.BOING_SKIP_DUMP === '1' || process.env.BOING_SKIP_DUMP === 'true';
  if (!skipDump) {
    const d = runNodeScript('dump-native-bytecodes.mjs', [], {});
    if (!d.ok) {
      console.error(d.stderr || d.stdout);
      exitAfterLog(d.status ?? 1);
    }
  }

  const secret = hexToBytes(validateHex32(secretHex.trim()));
  const client = createClient(rpc);
  const senderHexForWait = await senderHexFromSecretKey(secret);

  /** @type {{ skipEnv: string; artifact: string; saltKey: string; id: string }[]} */
  const steps = [
    {
      skipEnv: 'BOING_LP_AUX_SKIP_VAULT',
      artifact: 'native-amm-lp-vault.hex',
      saltKey: 'native_amm_lp_vault_v1',
      id: 'ammLpVault',
    },
    {
      skipEnv: 'BOING_LP_AUX_SKIP_SHARE',
      artifact: 'native-lp-share-token.hex',
      saltKey: 'native_lp_share_token_v1',
      id: 'lpShareToken',
    },
  ];

  /** @type {Record<string, unknown>} */
  const results = {};

  for (const step of steps) {
    if (process.env[step.skipEnv] === '1' || process.env[step.skipEnv] === 'true') {
      results[step.id] = { skipped: true, reason: `${step.skipEnv}=1` };
      continue;
    }

    const artifactPath = path.join(artifactsDir, step.artifact);
    if (!existsSync(artifactPath)) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: 'missing_artifact',
            path: artifactPath,
            hint: 'Run npm run dump-native-bytecodes or BOING_SKIP_DUMP=0',
          },
          null,
          2
        )
      );
      exitAfterLog(1);
    }

    const salt = saltForKey(step.saltKey);
    const bytecodeFile = artifactPath;

    const acc0 = await getAccountScheduled(client, rpc, senderHexForWait);
    const nonceBefore = BigInt(acc0.nonce);

    const baseEnv = {
      BOING_SECRET_HEX: secretHex.trim(),
      BOING_NATIVE_BYTECODE_FILE: bytecodeFile,
      BOING_CREATE2_SALT_HEX: salt,
    };

    console.warn(`[deploy-native-dex-lp-aux] ${step.id} (CREATE2)…`);
    let out = runNodeScript('deploy-native-purpose-contract.mjs', [], baseEnv);
    let retriedNonce = false;
    if (!out.ok && !userCreate2Off && !noAutoNonce && isCreate2Collision(out)) {
      console.warn(`[deploy-native-dex-lp-aux] ${step.id}: CREATE2 occupied — retry with BOING_USE_CREATE2=0`);
      out = runNodeScript('deploy-native-purpose-contract.mjs', [], { ...baseEnv, BOING_USE_CREATE2: '0' });
      retriedNonce = true;
    }
    if (!out.ok) {
      console.error(out.stderr || out.stdout);
      exitAfterLog(out.status ?? 1);
    }

    let json;
    try {
      json = parseDeployJson(out.stdout);
    } catch {
      console.error(out.stdout);
      exitAfterLog(1);
    }

    results[step.id] = { ...json, create2RetriedWithNonce: retriedNonce };

    await waitNonceAfter(client, senderHexForWait, nonceBefore, step.id);
  }

  console.log(JSON.stringify({ ok: true, rpc, results }, null, 2));
  scheduleExit(0);
}
