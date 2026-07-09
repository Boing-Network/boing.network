/**
 * Minimal Boing dApp starter: RPC client + native DEX discovery facade.
 *
 *   cd boing-sdk && npm install && npm run build
 *   cd ../examples/boing-dapp-starter && npm install && npm start
 */

import {
  createClient,
  createNativeDexDiscovery,
  probeBoingRpcCapabilities,
} from 'boing-sdk';

function env(name: string): string | undefined {
  const v = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process
    ?.env?.[name];
  return v != null && String(v).trim() ? String(v).trim() : undefined;
}

function rpcUrl(): string {
  return (
    env('BOING_RPC_URL') ||
    env('VITE_BOING_RPC_URL') ||
    env('REACT_APP_BOING_RPC_URL') ||
    'https://testnet-rpc.boing.network'
  );
}

async function main(): Promise<void> {
  const url = rpcUrl();
  console.log(`RPC: ${url}`);

  const client = createClient(url);
  const caps = await probeBoingRpcCapabilities(client);
  const available = Object.entries(caps.methods)
    .filter(([, p]) => p.available)
    .map(([k]) => k);
  console.log('RPC capabilities:', {
    clientVersion: caps.clientVersion,
    supportedMethodsCount: caps.supportedMethods?.length ?? null,
    availableCore: available,
  });

  const discovery = await createNativeDexDiscovery(client, {
    useProcessEnvOverrides: true,
  });

  console.log('Network:', {
    chainId: discovery.networkInfo.chain_id,
    chainName: discovery.networkInfo.chain_name,
    head: discovery.networkInfo.head_height,
  });
  console.log('DEX defaults (sources):', {
    pool: discovery.defaults.nativeCpPoolAccountHex,
    poolSource: discovery.defaults.poolSource,
    factory: discovery.defaults.nativeDexFactoryAccountHex,
    factorySource: discovery.defaults.factorySource,
  });
  console.log('Discovery:', {
    preferredPoolSource: discovery.preferredPoolSource,
    rpcPoolsSupported: discovery.rpcPoolsSupported,
    directoryBaseUrl: discovery.directoryBaseUrl,
  });

  const collected = await discovery.collectAllPools({ light: true });
  console.log(`Pools via ${collected.source}: ${collected.pools.length}`);
  if (collected.pools.length > 0) {
    const sample = collected.pools[0] as { poolHex?: string; pool?: string };
    console.log('First pool:', sample.poolHex ?? sample.pool ?? sample);
  }
}

main().catch((err) => {
  console.error(err);
  const proc = (globalThis as { process?: { exitCode?: number } }).process;
  if (proc) proc.exitCode = 1;
});
