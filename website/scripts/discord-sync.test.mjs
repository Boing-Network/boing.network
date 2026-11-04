import test from 'node:test';
import assert from 'node:assert/strict';
import { roleConfig, desiredRoles, planRoleChanges, discordSyncEnabled } from '../../workers/credibility-indexer/src/discord.js';
import { maskUsername } from '../functions/api/portal/_lib/discordOAuth.js';

const R = (n) => String(100000000000000000n + BigInt(n));
const env = { DISCORD_BOT_TOKEN: 'x', DISCORD_GUILD_ID: R(0), DISCORD_ROLE_BRONZE: R(1), DISCORD_ROLE_SILVER: R(2), DISCORD_ROLE_GOLD: R(3), DISCORD_ROLE_DEVELOPER_SILVER: R(12), DISCORD_ROLE_USER_GOLD: 'bad' };

test('disabled until token, guild and a role are set', () => {
  assert.equal(discordSyncEnabled({}), false);
  assert.equal(discordSyncEnabled({ ...env, DISCORD_BOT_TOKEN: '' }), false);
  assert.equal(discordSyncEnabled({ DISCORD_BOT_TOKEN: 'x', DISCORD_GUILD_ID: R(0) }), false);
  assert.equal(discordSyncEnabled(env), true);
});

test('only highest overall tier role + per-track roles; invalid ids ignored', () => {
  const cfg = roleConfig(env);
  assert.equal(cfg.all.size, 4);
  const want = desiredRoles([{ track: 'user', tier: 'bronze' }, { track: 'developer', tier: 'silver' }], cfg);
  assert.deepEqual([...want].sort(), [R(12), R(2)].sort());
  assert.equal(desiredRoles([{ track: 'user', tier: 'none' }], cfg).size, 0);
});

test('plan adds missing, removes stale managed, forgets unconfigured', () => {
  const cfg = roleConfig(env);
  const p = planRoleChanges(new Set([R(2)]), new Set([R(1), R(99)]), cfg);
  assert.deepEqual(p.add, [R(2)]);
  assert.deepEqual(p.remove, [R(1)]);
  assert.deepEqual(p.forget, [R(99)]);
});

test('username masking', () => {
  assert.equal(maskUsername('nicobuilds'), 'ni•••ds');
  assert.equal(maskUsername('abc'), 'a•••');
});
