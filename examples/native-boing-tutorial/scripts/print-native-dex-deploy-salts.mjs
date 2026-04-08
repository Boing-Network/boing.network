#!/usr/bin/env node
/**
 * Print canonical CREATE2 salt hex strings for native DEX / LP helper contracts (boing-sdk).
 * Use with deploy-native-purpose-contract: BOING_CREATE2_SALT_HEX=<value>.
 *
 * Usage:
 *   npm run print-native-dex-deploy-salts
 *       → full JSON (create2SaltsHex32 map).
 *   npm run print-native-dex-deploy-salts -- native_dex_ledger_router_v1
 *   node scripts/print-native-dex-deploy-salts.mjs native_dex_ledger_router_v1
 *       → single line: 0x + 64 hex (for `export BOING_CREATE2_SALT_HEX=$(...)` on Git Bash / Windows).
 *
 * Requires: cd ../../boing-sdk && npm run build
 */
import {
  nativeAmmLpVaultCreate2SaltV1Hex,
  nativeDexFactoryCreate2SaltV1Hex,
  nativeDexLedgerRouterCreate2SaltV1Hex,
  nativeDexLedgerRouterCreate2SaltV2Hex,
  nativeDexLedgerRouterCreate2SaltV3Hex,
  nativeDexMultihopSwapRouterCreate2SaltV1Hex,
  nativeLpShareTokenCreate2SaltV1Hex,
} from 'boing-sdk';

const out = {
  native_dex_factory_v1: nativeDexFactoryCreate2SaltV1Hex(),
  native_dex_ledger_router_v1: nativeDexLedgerRouterCreate2SaltV1Hex(),
  native_dex_ledger_router_v2: nativeDexLedgerRouterCreate2SaltV2Hex(),
  native_dex_ledger_router_v3: nativeDexLedgerRouterCreate2SaltV3Hex(),
  native_dex_swap2_router: nativeDexMultihopSwapRouterCreate2SaltV1Hex(),
  native_amm_lp_vault_v1: nativeAmmLpVaultCreate2SaltV1Hex(),
  native_lp_share_token_v1: nativeLpShareTokenCreate2SaltV1Hex(),
};

const keyArg = process.argv[2];
if (keyArg === '-h' || keyArg === '--help') {
  console.log(
    [
      'Usage:',
      '  print-native-dex-deploy-salts           # JSON map',
      '  print-native-dex-deploy-salts <key>     # one line 0x… salt',
      '',
      'Keys: ' + Object.keys(out).join(', '),
    ].join('\n')
  );
  process.exit(0);
}

if (keyArg != null && keyArg !== '') {
  const v = out[keyArg];
  if (v == null) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'unknown_salt_key',
          key: keyArg,
          knownKeys: Object.keys(out),
        },
        null,
        2
      )
    );
    process.exit(1);
  }
  process.stdout.write(`${v}\n`);
  process.exit(0);
}

console.log(JSON.stringify({ ok: true, create2SaltsHex32: out }, null, 2));
