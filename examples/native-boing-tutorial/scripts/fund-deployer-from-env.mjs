#!/usr/bin/env node
/**
 * Create / fund your deployer account on-chain via **`boing_faucetRequest`** (same RPC as deploy).
 *
 * **Why:** `boing_getAccount` can show balance `0` / nonce `0` even when your account is **not** in the
 * state trie yet. Deploy simulation runs the VM, which requires the sender account to **exist** — otherwise
 * you see **`Account not found`**. A successful faucet (or any incoming transfer) inserts the account.
 *
 * Loads **`.env`** from the tutorial root (same rules as `deploy-native-dex-full-stack.mjs`).
 *
 * Env:
 *   BOING_SECRET_HEX — required
 *   BOING_RPC_URL    — optional (default public testnet)
 */
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  createClient,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  validateHex32,
} from '@boing-network/boing-sdk';
import { loadDotEnvFile } from './tutorial-dotenv.mjs';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tutorialRoot = path.resolve(scriptDir, '..');

loadDotEnvFile(path.join(tutorialRoot, '.env'));

const secretRaw = process.env.BOING_SECRET_HEX?.trim();
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';

if (!secretRaw) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'missing_BOING_SECRET_HEX',
        hint: 'Set BOING_SECRET_HEX in .env',
      },
      null,
      2
    )
  );
  process.exit(1);
}

let secretHex;
try {
  secretHex = validateHex32(secretRaw);
} catch (e) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'invalid_BOING_SECRET_HEX',
        message: String(e?.message ?? e),
      },
      null,
      2
    )
  );
  process.exit(1);
}

async function main() {
  const secret = hexToBytes(secretHex);
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);

  const expect = process.env.BOING_EXPECT_SENDER_HEX?.trim();
  if (expect) {
    const want = validateHex32(expect.startsWith('0x') ? expect : `0x${expect}`);
    if (want !== senderHex) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: 'BOING_EXPECT_SENDER_HEX does not match BOING_SECRET_HEX',
            derivedSenderHex: senderHex,
            BOING_EXPECT_SENDER_HEX: want,
            hint: 'Regenerate keypair or fix .env — secret and expect sender must correspond.',
          },
          null,
          2
        )
      );
      process.exit(1);
    }
  }

  try {
    const out = await client.faucetRequest(senderHex);
    console.log(
      JSON.stringify(
        {
          ok: true,
          rpc,
          senderHex,
          faucet: out,
          next: 'Account funded on this RPC. Continue with deploy scripts (e.g. npm run deploy-native-dex-full-stack).',
        },
        null,
        2
      )
    );
  } catch (e) {
    const msg = explainBoingRpcError(e);
    console.error(
      JSON.stringify(
        {
          ok: false,
          rpc,
          senderHex,
          error: msg,
          hint:
            'Public RPC must expose boing_faucetRequest (node --faucet-enable). If this method is missing or rate-limited, use https://boing.network/faucet with the same RPC and paste senderHex above.',
        },
        null,
        2
      )
    );
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
