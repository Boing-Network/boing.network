# Contributing to Boing Network

Canonical repository: **[github.com/Boing-Network/boing.network](https://github.com/Boing-Network/boing.network)**.

## Quick start

```bash
cargo build
cargo test
```

TypeScript SDK:

```bash
cd boing-sdk && npm ci && npm run build && npm test
```

Tutorial scripts (after SDK build):

```bash
cd examples/native-boing-tutorial && npm ci
npm run preflight-rpc
```

## Documentation

- **Index:** [docs/README.md](docs/README.md)
- **Technical reference:** [docs/TECHNICAL-SPECIFICATION.md](docs/TECHNICAL-SPECIFICATION.md), [docs/RPC-API-SPEC.md](docs/RPC-API-SPEC.md)
- **Cross-repo consumers** (wallet, explorer, partners): [docs/HANDOFF-DEPENDENT-PROJECTS.md](docs/HANDOFF-DEPENDENT-PROJECTS.md), [docs/THREE-CODEBASE-ALIGNMENT.md](docs/THREE-CODEBASE-ALIGNMENT.md), [docs/HANDOFF_Boing_Network_Global_Token_Discovery.md](docs/HANDOFF_Boing_Network_Global_Token_Discovery.md), [docs/HANDOFF_DexDiscovery_Consumer_Repos.md](docs/HANDOFF_DexDiscovery_Consumer_Repos.md)
- **Universal contract deploy registry** (SDK + optional Cloudflare Worker): [docs/HANDOFF_Universal_Contract_Deploy_Indexer.md](docs/HANDOFF_Universal_Contract_Deploy_Indexer.md), `workers/deploy-registry-indexer/`

## Pull requests

- Keep changes focused; match existing style in touched files.
- For Rust: `cargo fmt` / `cargo clippy` as appropriate before pushing.
- For `boing-sdk`: run `npm run build` and `npm test` after TypeScript changes.
- Optional smoke against the deployed native DEX directory Worker (from repo root): `npm run verify-native-dex-directory-worker` — see [docs/HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md](docs/HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md).

## Website / Cloudflare

See [docs/WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md).
