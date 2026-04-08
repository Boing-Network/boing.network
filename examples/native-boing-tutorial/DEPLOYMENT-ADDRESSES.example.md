# Native DEX deployment addresses (local copy)

Copy to **`DEPLOYMENT-ADDRESSES.local.md`** (gitignored) and fill in. Do **not** commit signing seeds.

| Role | Account id (0x + 64 hex) | CREATE2 or nonce? |
|------|--------------------------|-------------------|
| Deployer (pubkey) | | |
| RPC URL | | |
| CP pool | | |
| Pair directory | | |
| Ledger router v1 | | |
| Ledger router v2 | | |
| Ledger router v3 | | |
| Multihop / swap2 router | | |
| LP vault (optional) | | |
| LP share token (optional) | | |

**`register_pair`:** token_a | token_b | pool

**Scripts used:** `bootstrap-native-pool-and-dex`, `deploy-native-purpose-contract`, `deploy-native-dex-aux-contracts`, `deploy-native-dex-lp-aux-contracts`, `dump-native-bytecodes`.

**Reference:** [docs/NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](../../docs/NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md)
