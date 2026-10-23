#!/usr/bin/env node
/**
 * Deploy native DEX **pair directory** bytecode (CREATE2 by default) and optionally `register_pair`.
 *
 * Prerequisites:
 *   - Funded deployer: BOING_SECRET_HEX
 *   - Bytecode: `cargo run -p boing-execution --example dump_native_dex_factory` → one `0x` line
 *
 * Env:
 *   BOING_RPC_URL                    — default https://testnet-rpc.boing.network
 *   BOING_SECRET_HEX                 — required
 *   BOING_DEX_FACTORY_BYTECODE_FILE  — path to hex file (one line), or
 *   BOING_DEX_FACTORY_BYTECODE_HEX   — inline 0x...
 *   BOING_USE_CREATE2                — default 1 (salt `NATIVE_DEX_FACTORY_CREATE2_SALT_V1`)
 *   BOING_PURPOSE                    — default dapp
 *   BOING_EXPECT_SENDER_HEX          — optional sanity check
 *
 * Optional second tx (`register_pair`):
 *   BOING_DEX_FACTORY_HEX            — directory contract (defaults to predicted CREATE2 address)
 *   BOING_DEX_POOL_HEX               — native CP pool account id
 *   BOING_DEX_TOKEN_A_HEX            — 32-byte token id (reference token or placeholder)
 *   BOING_DEX_TOKEN_B_HEX            — 32-byte token id
 *
 * Commit wait (`register_pair` runs after factory deploy submit; simulation needs the factory on-chain):
 *   BOING_DEX_REGISTER_POLL_MS       — default 750 — sleep between retries when sim says Account not found
 *   BOING_DEX_REGISTER_WAIT_MS       — default 180000 — max time to wait for factory to be executable
 */
import {
  BoingRpcError,
  buildNativeDexFactoryAccessList,
  createClient,
  encodeNativeDexRegisterPairCalldata,
  explainBoingRpcError,
  fetchNextNonce,
  hexToBytes,
  NATIVE_DEX_FACTORY_CREATE2_SALT_V1,
  predictNativeDexFactoryCreate2Address,
  predictNonceDerivedContractAddress,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
  submitDeployWithPurposeFlow,
  validateHex32,
} from '@boing-network/boing-sdk';
import { readFileSync } from 'node:fs';

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * `boing_simulateTransaction` uses committed state. Right after `submitTransaction` for the factory
 * deploy, the directory contract often does not exist yet → VM "Account not found". Poll until sim succeeds.
 */
async function submitRegisterPairWhenFactoryReady(opts) {
  const pollMs = Math.max(50, Number(process.env.BOING_DEX_REGISTER_POLL_MS ?? 750));
  const maxWaitMs = Math.max(pollMs, Number(process.env.BOING_DEX_REGISTER_WAIT_MS ?? 180_000));
  const deadline = Date.now() + maxWaitMs;
  /** @type {unknown} */
  let lastErr;
  let attempt = 0;
  while (Date.now() < deadline) {
    attempt += 1;
    try {
      const regOut = await submitContractCallWithSimulationRetry({
        ...opts,
        maxSimulationRetries: Number(process.env.BOING_DEX_REGISTER_SIM_RETRIES ?? 8),
      });
      return { ...regOut, registerPollAttempts: attempt };
    } catch (e) {
      lastErr = e;
      const msg = explainBoingRpcError(e);
      if (!/account not found/i.test(msg)) throw e;
      await sleep(pollMs);
    }
  }
  throw lastErr;
}

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const secretHex = process.env.BOING_SECRET_HEX;
const bytecodeFile = process.env.BOING_DEX_FACTORY_BYTECODE_FILE;
const bytecodeHexEnv = process.env.BOING_DEX_FACTORY_BYTECODE_HEX;
const useCreate2 = process.env.BOING_USE_CREATE2 !== '0' && process.env.BOING_USE_CREATE2 !== 'false';
const purposeCategory = process.env.BOING_PURPOSE ?? 'dapp';
const expectSender = process.env.BOING_EXPECT_SENDER_HEX?.trim();

const regPool = process.env.BOING_DEX_POOL_HEX?.trim();
const regTa = process.env.BOING_DEX_TOKEN_A_HEX?.trim();
const regTb = process.env.BOING_DEX_TOKEN_B_HEX?.trim();
const regFactory = process.env.BOING_DEX_FACTORY_HEX?.trim();

if (!secretHex) {
  console.error('Set BOING_SECRET_HEX (32-byte Ed25519 seed as 0x + 64 hex).');
  process.exit(1);
}

if (!bytecodeFile && !bytecodeHexEnv) {
  console.error(
    'Set BOING_DEX_FACTORY_BYTECODE_FILE or BOING_DEX_FACTORY_BYTECODE_HEX (from dump_native_dex_factory).'
  );
  process.exit(1);
}

function loadBytecodeHex() {
  const raw = bytecodeHexEnv
    ? bytecodeHexEnv.trim()
    : readFileSync(bytecodeFile, 'utf8').split(/\r?\n/).find((l) => l.trim().startsWith('0x')) ??
      readFileSync(bytecodeFile, 'utf8').trim();
  const line = raw
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => l.length > 0 && !l.startsWith('//'));
  if (!line) throw new Error('No bytecode line found');
  return line.startsWith('0x') ? line : `0x${line}`;
}

async function main() {
  const secret = hexToBytes(secretHex);
  const rawHex = loadBytecodeHex();
  const bytecode = hexToBytes(rawHex);

  const client = createClient(rpc);
  await client.chainHeight();

  const senderHex = await senderHexFromSecretKey(secret);
  if (expectSender) {
    const want = validateHex32(expectSender.startsWith('0x') ? expectSender : `0x${expectSender}`);
    if (want !== senderHex) {
      console.error(JSON.stringify({ ok: false, error: 'BOING_EXPECT_SENDER_HEX mismatch', want, senderHex }, null, 2));
      process.exit(1);
    }
  }

  const create2Salt = useCreate2 ? NATIVE_DEX_FACTORY_CREATE2_SALT_V1 : null;
  const deployNonce = await fetchNextNonce(client, senderHex);
  const predictedFactory = useCreate2
    ? predictNativeDexFactoryCreate2Address(senderHex, bytecode)
    : predictNonceDerivedContractAddress(senderHex, deployNonce);

  const out = await submitDeployWithPurposeFlow({
    client,
    secretKey32: secret,
    senderHex,
    bytecode,
    purposeCategory,
    create2Salt,
  });

  const factoryHex = regFactory ?? predictedFactory;
  /** @type {Record<string, unknown>} */
  const result = {
    ok: true,
    rpc,
    senderHex,
    purposeCategory,
    create2: useCreate2,
    predictedFactoryHex: predictedFactory,
    deploy_tx_hash: out.tx_hash,
    deploy_simulationAttempts: out.attempts,
  };

  if (regPool && regTa && regTb && factoryHex) {
    const fa = validateHex32(regTa.startsWith('0x') ? regTa : `0x${regTa}`);
    const fb = validateHex32(regTb.startsWith('0x') ? regTb : `0x${regTb}`);
    const fp = validateHex32(regPool.startsWith('0x') ? regPool : `0x${regPool}`);
    const ff = validateHex32(factoryHex.startsWith('0x') ? factoryHex : `0x${factoryHex}`);
    const calldata = encodeNativeDexRegisterPairCalldata(fa, fb, fp);
    const al = buildNativeDexFactoryAccessList(senderHex, ff);
    const regOut = await submitRegisterPairWhenFactoryReady({
      client,
      secretKey32: secret,
      senderHex,
      contractHex: ff,
      calldata,
      accessList: al,
    });
    result.register_tx_hash = regOut.tx_hash;
    result.register_simulationAttempts = regOut.attempts;
    if (regOut.registerPollAttempts > 1) {
      result.register_pollAttempts = regOut.registerPollAttempts;
    }
  } else if (regPool || regTa || regTb) {
    result.register_skipped =
      'Set all of BOING_DEX_FACTORY_HEX (or use CREATE2 prediction), BOING_DEX_POOL_HEX, BOING_DEX_TOKEN_A_HEX, BOING_DEX_TOKEN_B_HEX to register.';
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch((e) => {
  const msg = explainBoingRpcError(e);
  console.error(msg);
  if (/deployment address already has an account or code/i.test(msg)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'CREATE2 factory address is already occupied.',
          hint: 'Retry with BOING_USE_CREATE2=0 for a nonce-derived directory id, or call register_pair against the existing factory if you own it.',
        },
        null,
        2
      )
    );
  }
  if (e instanceof BoingRpcError && e.qaData?.rule_id === 'INVALID_OPCODE') {
    console.error(
      'Hint: upgrade the RPC node / testnet QA allowlist to match current boing-execution bytecode.'
    );
  }
  process.exit(1);
});
