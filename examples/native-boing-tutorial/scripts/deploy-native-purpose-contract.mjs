#!/usr/bin/env node
/**
 * Deploy arbitrary native Boing VM bytecode via ContractDeployWithPurpose (CREATE2 or nonce-derived).
 * Use for ledger routers, swap2 router, LP vault, LP share token, etc., after `npm run dump-native-bytecodes`.
 *
 * Env:
 *   BOING_SECRET_HEX            — required; 0x + 64 hex (Ed25519 seed)
 *   BOING_NATIVE_BYTECODE_FILE  — path to one-line 0x… hex file, or
 *   BOING_NATIVE_BYTECODE_HEX   — inline single-line 0x…
 *   BOING_USE_CREATE2           — default 1; set 0 for nonce-derived address
 *   BOING_CREATE2_SALT_HEX      — required when CREATE2: 0x + 64 hex (32-byte salt). Print values: `npm run print-native-dex-deploy-salts`
 *   BOING_RPC_URL               — default https://testnet-rpc.boing.network
 *   BOING_PURPOSE               — default dapp
 *   BOING_EXPECT_SENDER_HEX     — optional sanity check
 */
import {
  BoingRpcError,
  createClient,
  explainBoingRpcError,
  fetchNextNonce,
  hexToBytes,
  predictCreate2ContractAddress,
  predictNonceDerivedContractAddress,
  senderHexFromSecretKey,
  submitDeployWithPurposeFlow,
  validateHex32,
} from 'boing-sdk';
import { readFileSync } from 'node:fs';

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const secretRaw = process.env.BOING_SECRET_HEX;
const bytecodeFile = process.env.BOING_NATIVE_BYTECODE_FILE;
const bytecodeHexEnv = process.env.BOING_NATIVE_BYTECODE_HEX;
const saltHexEnv = process.env.BOING_CREATE2_SALT_HEX?.trim();
const useCreate2 = process.env.BOING_USE_CREATE2 !== '0' && process.env.BOING_USE_CREATE2 !== 'false';
const purposeCategory = process.env.BOING_PURPOSE ?? 'dapp';
const expectSender = process.env.BOING_EXPECT_SENDER_HEX?.trim();

function failJson(obj) {
  console.error(JSON.stringify(obj, null, 2));
  process.exit(1);
}

if (!secretRaw?.trim()) {
  failJson({ ok: false, error: 'missing_env:BOING_SECRET_HEX' });
}

let secretHex;
try {
  secretHex = validateHex32(secretRaw.trim());
} catch (e) {
  failJson({
    ok: false,
    error: 'invalid_BOING_SECRET_HEX',
    message: String(e?.message ?? e),
    hint: 'Must be 0x + exactly 64 hex characters.',
  });
}

if (!bytecodeFile && !bytecodeHexEnv) {
  failJson({
    ok: false,
    error: 'missing_bytecode',
    hint: 'Set BOING_NATIVE_BYTECODE_FILE or BOING_NATIVE_BYTECODE_HEX.',
  });
}

if (useCreate2 && !saltHexEnv) {
  failJson({
    ok: false,
    error: 'missing_env:BOING_CREATE2_SALT_HEX',
    hint: 'CREATE2 deploy requires a 32-byte salt hex. Run: npm run print-native-dex-deploy-salts',
  });
}

function loadBytecodeLine() {
  const text = bytecodeHexEnv?.trim() ?? readFileSync(bytecodeFile, 'utf8');
  const line = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => l.length > 0 && !l.startsWith('//') && l.startsWith('0x'));
  if (!line) throw new Error('No 0x bytecode line found');
  return line;
}

async function main() {
  const secret = hexToBytes(secretHex);
  const rawHex = loadBytecodeLine();
  const bytecode = hexToBytes(rawHex.startsWith('0x') ? rawHex : `0x${rawHex}`);

  const client = createClient(rpc);
  await client.chainHeight();

  const senderHex = await senderHexFromSecretKey(secret);
  if (expectSender) {
    const want = validateHex32(expectSender.startsWith('0x') ? expectSender : `0x${expectSender}`);
    if (want !== senderHex) {
      failJson({ ok: false, error: 'BOING_EXPECT_SENDER_HEX mismatch', want, senderHex });
    }
  }

  const create2Salt = useCreate2 ? hexToBytes(validateHex32(saltHexEnv)) : null;
  const deployNonce = await fetchNextNonce(client, senderHex);
  const predictedContract = useCreate2
    ? predictCreate2ContractAddress(senderHex, create2Salt, bytecode)
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
        purposeCategory,
        create2: useCreate2,
        predictedContractHex: predictedContract,
        tx_hash: out.tx_hash,
        simulationAttempts: out.attempts,
        note: useCreate2
          ? 'After inclusion, contract should be at predictedContractHex.'
          : 'Nonce-derived deploy; predictedContractHex matches BLAKE3(sender || nonce_le) for this tx.',
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  const msg = explainBoingRpcError(e);
  console.error(msg);
  if (/deployment address already has an account or code/i.test(msg)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'CREATE2 address already occupied.',
          hint: 'Retry with BOING_USE_CREATE2=0 or use a different deployer / bytecode / salt.',
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
      'Hint: RPC QA allowlist may be older than this bytecode; upgrade boing-node or use a matching testnet.'
    );
  }
  process.exit(1);
});
