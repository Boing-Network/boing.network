/**
 * Last published native DEX aux ids from the **previous** public testnet ledger (chain **6913**).
 * Hosted Fly RPC currently returns null `end_user.canonical_native_*` until ops re-runs
 * `npm run deploy-native-dex-full-stack` and sets `BOING_CANONICAL_NATIVE_*` on the nodes.
 *
 * **V1 ledger router** is optional (`BOING_AUX_INCLUDE_LEDGER_V1`).
 * `CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX` remains a legacy CREATE2 prediction id — do not assume it is deployed.
 */
import { validateHex32 } from './hex.js';
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX = validateHex32('0x3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29');
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX = validateHex32('0x58112627fc84618a27b82e9af82bc9a51761c6d3cca1260c93d56d22b6c481a1');
/** Legacy predicted v1 — not deployed on the current testnet-rpc full-stack bundle unless aux includes v1. */
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX = validateHex32('0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d');
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX = validateHex32('0x33334ff73c44c93335ac5e69938a52ea65fa77b062d1961ed22c131adaa31e0f');
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX = validateHex32('0x2c90ffcddeb2683219b4b8143a91d7b93f249bcb0d9523c8b4f2111de668b79a');
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX = validateHex32('0xf801cd1aa5ec402f89a2f394b49e6b0c136264d8945b16a4a6a81a188b18acc1');
export const CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX = validateHex32('0x937d09ee8e4dcc521c812566ad4930792e74ad004ecb3ae2cc73dc015813aa8d');
export const CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX = validateHex32('0x101201403f573e5b1d6d5c6b93d52d12c68957f4a228d5dad76e78c747044421');
