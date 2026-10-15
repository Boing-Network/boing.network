/**
 * Native DEX aux ids on the current public testnet ledger (chain **6913**), verified against
 * `boing_getNetworkInfo.end_user.canonical_native_*` and the CREATE2 deploy txs (heights 2-8) on 2026-10-15.
 * Live RPC values always win; these apply only when network info is unavailable.
 *
 * **V1 ledger router** is optional (`BOING_AUX_INCLUDE_LEDGER_V1`).
 * `CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX` is a legacy CREATE2 prediction id and is **not** deployed on the current ledger.
 */

import { validateHex32 } from './hex.js';

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX = validateHex32(
  '0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX = validateHex32(
  '0x7322ccec1ffdbf25d3f60384db0f9a38f8d6121233daccd48d4477d6036aff2a',
);

/** Legacy predicted v1 — not deployed on the current testnet-rpc full-stack bundle unless aux includes v1. */
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX = validateHex32(
  '0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX = validateHex32(
  '0xd39d7bb18e986a6e39a69521d9b6f1fe403ff757ee8d409b89f2bcccc4df7980',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX = validateHex32(
  '0x082a13685fbb69999f2d1378b2f704f3103ae6929b18a87510b513cb790c53c9',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX = validateHex32(
  '0xd8504729900a2d6dc8b8afc7bf24a18dc08732d24a4d2e3555ea5b7f28f3672c',
);

export const CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX = validateHex32(
  '0xac7a635931b192f5cff054e9994242f54fbda579a46eba21d9787600971ec90f',
);

export const CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX = validateHex32(
  '0x2f460046eb700ff8c7eebe6f4c409465a9111f32210edcc478a2a0de9d0ec2db',
);
