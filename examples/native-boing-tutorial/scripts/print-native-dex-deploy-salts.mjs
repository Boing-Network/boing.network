#!/usr/bin/env node
/**
 * Print canonical CREATE2 salt hex strings for native DEX / LP helper contracts (boing-sdk).
 * Use with deploy-native-purpose-contract: BOING_CREATE2_SALT_HEX=<value>.
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

console.log(JSON.stringify({ ok: true, create2SaltsHex32: out }, null, 2));
