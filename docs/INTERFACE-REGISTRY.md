# Interface registry (published contract interfaces)

A contract's **deployer** publishes the BLAKE3 hash and URI of an interface JSON so explorers can show
authoritative function names instead of decoded or guessed ones.

| | |
|---|---|
| Rust | `boing_execution::interface_registry` |
| SDK | `boing-sdk` `interfaceRegistry.ts` |
| CLI | `boing iface deploy / bytecode / hash / get / publish` |
| CREATE2 salt | `BOING_INTERFACE_REGISTRY_V1` |
| Public testnet | `0xbd7373b6a3830768b2cea3768bedc737b43d1f981123c83e7f0d75ebb310fb6c` (deployed by the canonical DEX deployer `0x8079f305…696e`, block 658) |

## Methods

Calldata: selector in the low byte of word 0, then 32-byte words.

| Selector | Method | Calldata | Notes |
|---|---|---|---|
| `0xD0` | `claim_asset(contract)` | 64 B | First caller becomes the claimer. Re-claim by the claimer is a no-op; anyone else is rejected. |
| `0xD1` | `set_interface(contract, interface_hash, uri_len, uri[6 words])` | 320 B | Claimer only. `interface_hash != 0`, `uri_len <= 192`. Records `publisher = CALLER`. Emits `Log3(BOING_IFACE_SET_V1, contract, interface_hash; data = URI words)`. |
| `0xD2` | `get_interface(contract)` | 64 B | Returns 320 B: claimer, publisher, interface_hash, uri_len, uri[6 words]. |
| `0xD3` | `get_asset_claimer(contract)` | 64 B | Returns the claimer word. |
| `0xD4` | `transfer_asset_claimer(contract, new_claimer)` | 96 B | Claimer only. |

Storage (readable with `boing_getContractStorage`, no simulation needed): claimer at
`contract ^ "BOING_IFACE_CLAIMER_V1"`, field `f` at `contract ^ ("BOING_IFACE_FIELD_V1" with byte 31 = f)`
(0 = hash, 1 = uri_len, 2..7 = URI words, 8 = publisher).

## Trust model

The VM cannot see who deployed a contract, so **`claim_asset` is first-caller**. A client must only show a record
as *Published by deployer* when:

1. the claimer equals the contract's deployer (the sender of the `contract_deploy` tx, proven by recomputing the
   contract address from that tx: nonce-derived or CREATE2), and
2. BLAKE3 of the bytes fetched from the URI equals `interface_hash`.

boing.observer does both; otherwise it ignores the record.

## Interface JSON

Same shape as the observer's built-in interfaces:

```json
{
  "schema": "boing-interface/v1",
  "network": "boing-testnet",
  "contract": "0x…",
  "name": "USDC / tBOING pool (native CP pool v9)",
  "functions": [
    { "selector": 16, "name": "swap", "params": [{ "name": "direction", "type": "u128" }],
      "kind": "write", "description": "…", "access": "…" }
  ]
}
```

`params[].type` is one of `account`, `u128`, `u64`, `bytes32`, `bool`, `u8`. The hash covers the exact bytes served,
so serve the file unchanged (the boing.network site serves them from `/interfaces/<network>/<contract>.json`).

## Publishing

```bash
export BOING_SECRET_HEX=…            # deployer key, env only
boing --rpc-url https://testnet-rpc.boing.network iface hash my-interface.json
boing --rpc-url https://testnet-rpc.boing.network iface publish 0xCONTRACT \
  --file my-interface.json --uri https://example.com/my-interface.json --dry-run
boing --rpc-url https://testnet-rpc.boing.network iface publish 0xCONTRACT \
  --file my-interface.json --uri https://example.com/my-interface.json
boing --rpc-url https://testnet-rpc.boing.network iface get 0xCONTRACT
```

Canonical testnet interfaces (USDC, USDC/tBOING v9 pool, custody router, LP vault v2) are published from
`website/public/interfaces/testnet/`.
## Built-in hosting on Boing (interface blob store)

Publishers don't need their own web hosting. The interface JSON can live on-chain in the
**interface blob store**, which is an ordinary contract that sits next to the registry. It needs no consensus change.

- Testnet store: `0xa524837577e67228bf676257e861d29e510a3eddd1a4dbd607f613bbcd37904d`
  (`boing iface deploy-blob-store`, bytecode via `boing iface blob-store-bytecode`).
- `put(id, start, count, words…)` (selector `0xE0`, 1–64 words per call). Each word is stored at
  `CALLER ^ TAG("BOING_IFACE_BLOB_V1") ^ (id << 32) ^ index`. A writer can only ever write its own namespace.
- `id` is the first 8 bytes of the interface hash, so each version gets fresh slots.
- Word 0 is a header: `"BIF1"`, an encoding byte (0 raw, 1 deflate-raw) and the length as a u64 BE in bytes 24..31.
  Data starts at word 1. The stored cap is 24 KB.
- URI: `boing://iface-blob/0x<writer 64 hex>/<id 16 hex>`. It fits in the registry's 192-byte URI field.
- Fee: normal gas, one SSTORE per 32 bytes, so the cost scales with size. Deflate usually shrinks the JSON 3–6×.
- The claim, the blob writes and `set` go out as ONE native batch (at most 16 calls).
- Observers read the words with `boing_getContractStorage`, inflate them, and accept the file only if its
  BLAKE3 matches the hash in the registry. That is the same check used for https URIs.
- SDK: `buildHostInterfaceBlobFlow`, `readInterfaceBlob`, `storageWordReaderFromRpc` (`boing-sdk`).

Why not a Cloudflare endpoint? R2/KV that checks hashes would work, but it adds infra, an account
dependency and an off-chain availability point. The on-chain store uses existing primitives, is verified by the
same hash check, and is paid for by the publisher in gas.
