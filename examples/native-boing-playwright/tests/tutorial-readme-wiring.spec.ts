/**
 * No browser: asserts the native-boing-tutorial README lists LP vault + LP share npm scripts
 * (guards against drift between docs and package.json).
 */
import * as fs from 'node:fs';
import * as path from 'node:path';
import { test, expect } from '@playwright/test';

const readmePath = path.join(__dirname, '../../native-boing-tutorial/README.md');

test.describe('Tutorial README wiring', () => {
  test('mentions LP vault and LP share npm scripts', () => {
    const text = fs.readFileSync(readmePath, 'utf8');
    for (const needle of [
      'native-amm-lp-vault-print-contract-call-tx',
      'native-amm-lp-vault-submit-contract-call',
      'native-lp-share-print-contract-call-tx',
      'native-lp-share-submit-contract-call',
      '### 7f.',
      '### 7h.',
    ]) {
      expect(text, `README should mention ${needle}`).toContain(needle);
    }
  });
});
