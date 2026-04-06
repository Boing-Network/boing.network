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
- **Cross-repo consumers** (wallet, explorer, partners): [docs/HANDOFF-DEPENDENT-PROJECTS.md](docs/HANDOFF-DEPENDENT-PROJECTS.md), [docs/THREE-CODEBASE-ALIGNMENT.md](docs/THREE-CODEBASE-ALIGNMENT.md)

## Pull requests

- Keep changes focused; match existing style in touched files.
- For Rust: `cargo fmt` / `cargo clippy` as appropriate before pushing.
- For `boing-sdk`: run `npm run build` and `npm test` after TypeScript changes.

## Website / Cloudflare

See [docs/WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md).
