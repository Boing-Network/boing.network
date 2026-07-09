# Boing dApp starter (TypeScript)

Minimal Node script that wires **`boing-sdk`**: RPC preflight, **`createNativeDexDiscovery`**, and a sample pool collect.

## Setup

```bash
# From repo root — rebuild SDK after source changes
cd boing-sdk && npm install && npm run build

cd ../examples/boing-dapp-starter
npm install
cp .env.example .env   # optional: BOING_RPC_URL, directory Worker URL
npm start
```

## What it prints

- `probeBoingRpcCapabilities` summary
- Chain id / tip from `getNetworkInfo`
- Merged native DEX defaults (pool / factory + sources)
- Preferred discovery source (`rpc` → `http` → `none`) and pool count from `collectAllPools`

## Next steps

- Browser wallet path: [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md)
- Deploy + seed a private DEX stack: from repo root `npm run seed-native-dex` (see [DEVNET-OPERATOR-NATIVE-AMM.md](../../docs/DEVNET-OPERATOR-NATIVE-AMM.md))
- Full tutorial scripts: [examples/native-boing-tutorial](../native-boing-tutorial/)
