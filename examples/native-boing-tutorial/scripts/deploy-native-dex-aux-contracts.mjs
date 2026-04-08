#!/usr/bin/env node
/**
 * Deploy remaining native DEX VM contracts for a full on-chain routing stack (after pool + directory + ledger v1).
 *
 * Deploys (default — each can be skipped with env):
 *   1. Multihop / swap2 router — 2–4 pool hops in one tx ([NATIVE-DEX-SWAP2-ROUTER.md](../../../docs/NATIVE-DEX-SWAP2-ROUTER.md))
 *   2. Ledger router v2 — forward 160-byte inner calldata (e.g. v5 swap_to) ([NATIVE-DEX-LEDGER-ROUTER.md](../../../docs/NATIVE-DEX-LEDGER-ROUTER.md))
 *   3. Ledger router v3 — forward 192-byte inner calldata (e.g. v5 remove_liquidity_to)
 *
 * Optional: ledger v1 if you need CREATE2 at canonical address on a fresh key (`BOING_AUX_INCLUDE_LEDGER_V1=1`).
 *
 * Prerequisites:
 *   - `npm run dump-native-bytecodes` (or `BOING_SKIP_DUMP=1` with `artifacts/*.hex` present)
 *   - `boing-sdk` built; `BOING_SECRET_HEX`
 *
 * Env:
 *   BOING_SECRET_HEX, BOING_RPC_URL — required / forwarded
 *   BOING_SKIP_DUMP — `1` to skip running dump-native-bytecodes.mjs first
 *   BOING_AUX_SKIP_SWAP2, BOING_AUX_SKIP_LEDGER_V2, BOING_AUX_SKIP_LEDGER_V3 — `1` to skip that deploy
 *   BOING_AUX_INCLUDE_LEDGER_V1 — `1` to also deploy ledger v1 (default off; you may already have it)
 *   BOING_BOOTSTRAP_NO_AUTO_NONCE — `1` to disable CREATE2-collision → nonce retry
 *   BOING_AUX_COMMIT_WAIT_MS — max ms to wait for sender nonce after each tx (default 120000)
 */
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createClient, hexToBytes, senderHexFromSecretKey, validateHex32 } from 'boing-sdk';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tutorialRoot = path.resolve(scriptDir, '..');
const artifactsDir = path.join(tutorialRoot, 'artifacts');

const secretHex = process.env.BOING_SECRET_HEX;
if (!secretHex?.trim()) {
  console.error('Set BOING_SECRET_HEX (0x + 64 hex).');
  process.exit(1);
}

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const noAutoNonce =
  process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === '1' || process.env.BOING_BOOTSTRAP_NO_AUTO_NONCE === 'true';
const userCreate2Off = process.env.BOING_USE_CREATE2 === '0' || process.env.BOING_USE_CREATE2 === 'false';
const commitWaitMs = Number(process.env.BOING_AUX_COMMIT_WAIT_MS ?? 120_000);

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
    const acc = await client.getAccount(senderHex);
    if (BigInt(acc.nonce) > nonceBefore) return;
    await sleep(400);
  }
  throw new Error(`Timeout waiting for nonce to advance after ${label}`);
}

function saltForKey(saltKey) {
  const r = runNodeScript('print-native-dex-deploy-salts.mjs', [saltKey], {});
  if (!r.ok) {
    console.error(r.stderr || r.stdout);
    process.exit(r.status ?? 1);
  }
  const line = r.stdout
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => /^0x[0-9a-f]{64}$/i.test(l));
  if (!line) {
    console.error(JSON.stringify({ ok: false, error: 'bad_salt_output', saltKey, stdout: r.stdout }, null, 2));
    process.exit(1);
  }
  return line;
}

async function main() {
  const skipDump = process.env.BOING_SKIP_DUMP === '1' || process.env.BOING_SKIP_DUMP === 'true';
  if (!skipDump) {
    const d = runNodeScript('dump-native-bytecodes.mjs', [], {});
    if (!d.ok) {
      console.error(d.stderr || d.stdout);
      process.exit(d.status ?? 1);
    }
  }

  const secret = hexToBytes(validateHex32(secretHex.trim()));
  const client = createClient(rpc);
  const senderHexForWait = await senderHexFromSecretKey(secret);

  /** @type {{ skipEnv: string; artifact: string; saltKey: string; id: string }[]} */
  const steps = [];
  if (process.env.BOING_AUX_INCLUDE_LEDGER_V1 === '1' || process.env.BOING_AUX_INCLUDE_LEDGER_V1 === 'true') {
    steps.push({
      skipEnv: 'BOING_AUX_SKIP_LEDGER_V1',
      artifact: 'native-dex-ledger-router-v1.hex',
      saltKey: 'native_dex_ledger_router_v1',
      id: 'ledgerRouterV1',
    });
  }
  steps.push(
    {
      skipEnv: 'BOING_AUX_SKIP_SWAP2',
      artifact: 'native-dex-swap2-router.hex',
      saltKey: 'native_dex_swap2_router',
      id: 'swap2MultihopRouter',
    },
    {
      skipEnv: 'BOING_AUX_SKIP_LEDGER_V2',
      artifact: 'native-dex-ledger-router-v2.hex',
      saltKey: 'native_dex_ledger_router_v2',
      id: 'ledgerRouterV2',
    },
    {
      skipEnv: 'BOING_AUX_SKIP_LEDGER_V3',
      artifact: 'native-dex-ledger-router-v3.hex',
      saltKey: 'native_dex_ledger_router_v3',
      id: 'ledgerRouterV3',
    }
  );

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
      process.exit(1);
    }

    const salt = saltForKey(step.saltKey);
    const bytecodeFile = artifactPath;

    const nonceBefore = BigInt((await client.getAccount(senderHexForWait)).nonce);

    const baseEnv = {
      BOING_SECRET_HEX: secretHex.trim(),
      BOING_NATIVE_BYTECODE_FILE: bytecodeFile,
      BOING_CREATE2_SALT_HEX: salt,
    };

    console.warn(`[deploy-native-dex-aux] ${step.id} (CREATE2)…`);
    let out = runNodeScript('deploy-native-purpose-contract.mjs', [], baseEnv);
    let retriedNonce = false;
    if (!out.ok && !userCreate2Off && !noAutoNonce && isCreate2Collision(out)) {
      console.warn(`[deploy-native-dex-aux] ${step.id}: CREATE2 occupied — retry with BOING_USE_CREATE2=0`);
      out = runNodeScript('deploy-native-purpose-contract.mjs', [], { ...baseEnv, BOING_USE_CREATE2: '0' });
      retriedNonce = true;
    }
    if (!out.ok) {
      console.error(out.stderr || out.stdout);
      process.exit(out.status ?? 1);
    }

    let json;
    try {
      json = parseDeployJson(out.stdout);
    } catch {
      console.error(out.stdout);
      process.exit(1);
    }

    results[step.id] = { ...json, create2RetriedWithNonce: retriedNonce };

    try {
      await waitNonceAfter(client, senderHexForWait, nonceBefore, step.id);
    } catch (e) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: String(e?.message ?? e),
            phase: 'wait_nonce',
            lastDeploy: step.id,
            hint: 'Ensure blocks are produced; increase BOING_AUX_COMMIT_WAIT_MS',
          },
          null,
          2
        )
      );
      process.exit(1);
    }
  }

  console.log(JSON.stringify({ ok: true, rpc, results }, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
