// Publishes the Testnet Passport rules (single source: src/config/passport-rules-v1.json)
// as a static file at /testnet/passport/rules-v1.json.
import { mkdirSync, copyFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'public', 'testnet', 'passport', 'rules-v1.json');
mkdirSync(dirname(out), { recursive: true });
copyFileSync(join(root, 'src', 'config', 'passport-rules-v1.json'), out);
console.log('passport rules ->', out);
