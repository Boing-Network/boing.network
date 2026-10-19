#!/usr/bin/env node
/**
 * Deploy the linked NFT↔token registry **v2** (creator-only claims) on the public testnet, seeded with
 * the v1 claims and links plus a deployer record for every contract that existed at the copy height
 * (`scripts/linked-nft-token-registry-v2-testnet.seed.txt`).
 *
 *   cargo run -p boing-execution --example dump_linked_nft_token_registry_v2 -- \
 *     --seed scripts/linked-nft-token-registry-v2-testnet.seed.txt > /tmp/reg_v2_deploy.hex
 *   node scripts/deploy-linked-nft-token-registry-v2.mjs --bytecode-file /tmp/reg_v2_deploy.hex           # dry run
 *   BOING_SECRET_HEX=0x... node scripts/deploy-linked-nft-token-registry-v2.mjs --bytecode-file ... --send
 *
 * Before deploying it re-reads v1 and refuses when v1 claims or links differ from the seed.
 * After deploying it checks the stored code (== v2 runtime), every seeded slot and the CREATE2 address
 * against CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX. Never commit or print the secret.
 */
import fs from 'node:fs';
import {
  CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
  CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX,
  DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX,
  LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX,
  LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX,
  createClient,
  hexToAccountId,
  hexToBytes,
  linkedNftTokenClaimerStorageKeyHex,
  linkedNftTokenDeployerStorageKeyHex,
  predictCreate2ContractAddress,
  senderHexFromSecretKey,
  signTransactionInputForClient,
  submitSignedTransactionIdempotent,
  transactionIdFromSignedTransactionHex,
  validateHex32,
} from '../boing-sdk/dist/index.js';

const args = process.argv.slice(2);
const opt = (k, d) => {
  const i = args.indexOf(k);
  return i >= 0 ? args[i + 1] : d;
};
const send = args.includes('--send');
const seedPath = opt('--seed', new URL('./linked-nft-token-registry-v2-testnet.seed.txt', import.meta.url));
const bytecodeFile = opt('--bytecode-file');
if (!bytecodeFile) throw new Error('pass --bytecode-file (output of dump_linked_nft_token_registry_v2 --seed ...)');
const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/';
const client = createClient(rpc);
const lc = (h) => (h ?? '').toLowerCase();
const ZERO = `0x${'00'.repeat(32)}`;

const seed = { claims: [], links: [], deployers: [] };
for (const raw of fs.readFileSync(seedPath, 'utf8').split('\n')) {
  const line = raw.split('#')[0].trim();
  if (!line) continue;
  const [kind, a, b] = line.split(/\s+/);
  const pair = [lc(validateHex32(a)), lc(validateHex32(b))];
  if (kind === 'claim') seed.claims.push(pair);
  else if (kind === 'link') seed.links.push(pair);
  else if (kind === 'deployer') seed.deployers.push(pair);
  else throw new Error(`bad seed line: ${line}`);
}
const bytecodeHex = fs.readFileSync(bytecodeFile, 'utf8').trim();
const v1 = CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX;
const storage = async (contract, key) => lc((await client.getContractStorage(contract, key)).value);
const pairKey = (i, f) => {
  const base = BigInt(`0x${Buffer.from('BOINGLNKPAIR').toString('hex')}${'00'.repeat(20)}`);
  return `0x${(base + BigInt(i) * 2n + BigInt(f)).toString(16).padStart(64, '0')}`;
};

// 1. v1 still matches the seed (otherwise regenerate the seed first).
for (const [asset, claimer] of seed.claims) {
  const now = await storage(v1, linkedNftTokenClaimerStorageKeyHex(asset));
  if (now !== claimer) throw new Error(`v1 claimer of ${asset} is ${now}, seed says ${claimer}`);
}
const v1Count = Number(BigInt(await storage(v1, LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX)));
if (v1Count !== seed.links.length) throw new Error(`v1 has ${v1Count} links, seed has ${seed.links.length}`);
for (let i = 0; i < v1Count; i++) {
  const c = await storage(v1, pairKey(i, 0));
  const t = await storage(v1, pairKey(i, 1));
  if (c !== seed.links[i][0] || t !== seed.links[i][1]) throw new Error(`v1 link ${i} differs from the seed`);
}
console.log(`v1 ${v1}: ${seed.claims.length} claims and ${v1Count} links match the seed; ${seed.deployers.length} deployer records`);

// 2. plan
const secretHex = process.env.BOING_SECRET_HEX?.trim();
if (send && !secretHex) throw new Error('--send needs BOING_SECRET_HEX (never print it)');
const secret = secretHex ? hexToBytes(secretHex) : null;
const senderHex = lc(secret ? await senderHexFromSecretKey(secret) : opt('--sender', ''));
if (!senderHex) throw new Error('pass --sender 0x... for a read-only run, or BOING_SECRET_HEX');
const predicted = lc(predictCreate2ContractAddress(senderHex, hexToBytes(LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX), hexToBytes(bytecodeHex)));
console.log(`deployer ${senderHex}; v2 address ${predicted} (${(bytecodeHex.length - 2) / 2} bytes)`);
if (predicted !== lc(CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX)) {
  console.log(`  note: differs from SDK CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX ${CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX}`);
}
const qa = await client.qaCheck(bytecodeHex, 'dapp');
console.log(`qaCheck: ${qa.result}`);
if (qa.result === 'reject') throw new Error('QA rejected the registry bytecode');

const existing = await client.getContractCode(predicted).catch(() => null);
const alreadyDeployed = Boolean(existing?.bytecode && existing.bytecode !== '0x');
if (alreadyDeployed) console.log('already deployed; verifying only');

if (secret && !alreadyDeployed) {
  const sender = hexToAccountId(senderHex);
  const nonce = BigInt((await client.getAccount(senderHex)).nonce);
  const signed = await signTransactionInputForClient(
    client,
    {
      nonce,
      sender,
      payload: {
        kind: 'contractDeployWithPurposeAndMetadata',
        bytecode: hexToBytes(bytecodeHex),
        purposeCategory: 'dapp',
        descriptionHash: null,
        assetName: 'Linked NFT Token Registry v2',
        assetSymbol: 'LNTR2',
        create2Salt: hexToBytes(LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX),
      },
      accessList: { read: [sender], write: [sender] },
    },
    secret,
  );
  const id = transactionIdFromSignedTransactionHex(signed);
  const sim = await client.simulateTransaction(signed);
  console.log(`deploy ${id} simulate success=${sim.success} gas=${sim.gas_used}${sim.error ? ` (${sim.error})` : ''}`);
  if (!sim.success) throw new Error('simulation failed');
  if (!send) {
    console.log('[dry-run] nothing submitted');
    process.exit(0);
  }
  console.log(`submit: ${await submitSignedTransactionIdempotent(client, signed)}`);
  let rc = null;
  for (let i = 0; i < 60 && !rc; i++) {
    rc = await client.getTransactionReceipt(id);
    if (!rc) await new Promise((r) => setTimeout(r, 2000));
  }
  if (!rc?.success) throw new Error(`deploy failed: ${rc?.error ?? 'no receipt'}`);
  console.log(`included block ${rc.block_height} success=${rc.success} gas=${rc.gas_used}`);
} else if (!alreadyDeployed) {
  console.log('[read-only] no key given; nothing signed');
  process.exit(0);
}

// 3. verify on chain
const code = await client.getContractCode(predicted);
const okCode = lc(code?.bytecode) === lc(DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX);
let bad = 0;
for (const [asset, claimer] of seed.claims) bad += (await storage(predicted, linkedNftTokenClaimerStorageKeyHex(asset))) === claimer ? 0 : 1;
for (const [asset, dep] of seed.deployers) bad += (await storage(predicted, linkedNftTokenDeployerStorageKeyHex(asset))) === dep ? 0 : 1;
for (let i = 0; i < seed.links.length; i++) {
  bad += (await storage(predicted, pairKey(i, 0))) === (seed.links[i][0] || ZERO) ? 0 : 1;
  bad += (await storage(predicted, pairKey(i, 1))) === (seed.links[i][1] || ZERO) ? 0 : 1;
}
const count = Number(BigInt(await storage(predicted, LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX)));
console.log(`stored code == v2 runtime: ${okCode ? 'OK' : 'MISMATCH'}`);
console.log(`seeded slots: ${bad === 0 ? 'all OK' : `${bad} MISMATCH`} (links_count ${count})`);
if (!okCode || bad || count !== seed.links.length) process.exit(1);
