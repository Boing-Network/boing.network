import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { syncDeployRegistry } from '../src/index';

/** Minimal in-memory D1 covering only the statements the indexer issues. */
function fakeD1() {
  const state = new Map<string, string>();
  const stats = { writes: 0, inserts: 0 };
  const keyFromSql = (sql: string) => /VALUES \('(\w+)'/.exec(sql)?.[1];
  function stmt(sql: string, args: unknown[] = []) {
    const s = {
      bind: (...a: unknown[]) => stmt(sql, a),
      first: async () => {
        const k = /WHERE k = '(\w+)'/.exec(sql)?.[1] ?? (args[0] as string);
        return state.has(k) ? { v: state.get(k) } : null;
      },
      run: async () => exec(),
      all: async () => ({ results: [] }),
      exec,
    };
    function exec() {
      if (/INSERT INTO ingest_state/.test(sql)) {
        state.set(keyFromSql(sql)!, String(args[0]));
        stats.writes += 1;
      } else if (/INSERT OR IGNORE INTO contract_deployments/.test(sql)) {
        stats.inserts += 1;
        return { meta: { changes: 1 } };
      }
      return { meta: { changes: 1 } };
    }
    return s;
  }
  return {
    state,
    stats,
    db: {
      prepare: (sql: string) => stmt(sql),
      batch: async (stmts: ReturnType<typeof stmt>[]) => stmts.map((s) => s.exec()),
    } as unknown as D1Database,
  };
}

const OVER = 'Upstream RPC response exceeded gateway limit (6291456 bytes).';
type Behavior = 'ok' | 'over' | 'http503' | 'timeout';

function installGateway(tip: number, behave: (h: number) => Behavior) {
  const calls: number[] = [];
  vi.stubGlobal(
    'fetch',
    vi.fn(async (_url: unknown, init: { body: string }) => {
      const body = JSON.parse(init.body) as { id: number; method: string; params: unknown[] };
      const rpc = (extra: object) => new Response(JSON.stringify({ jsonrpc: '2.0', id: body.id, ...extra }), { status: 200 });
      if (body.method === 'boing_chainHeight') return rpc({ result: tip });
      const h = body.params[0] as number;
      calls.push(h);
      const b = behave(h);
      if (b === 'over') return rpc({ error: { code: -32000, message: OVER } });
      if (b === 'http503') return new Response('error code: 1102', { status: 503 });
      if (b === 'timeout') throw new Error('fetch failed');
      return rpc({ result: { hash: `h${h}`, header: { height: h }, transactions: [] } });
    }),
  );
  return calls;
}

const mkEnv = (db: D1Database) => ({
  DEPLOY_REGISTRY_DB: db,
  DEPLOY_REGISTRY_RPC_URL: 'https://rpc.test',
  DEPLOY_REGISTRY_FROM_HEIGHT: '0',
  DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK: '20',
  DEPLOY_REGISTRY_PARALLEL_FETCHES: '6',
});

describe('syncDeployRegistry', () => {
  beforeEach(() => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('advances past oversize blocks (1311..1316 shape) and records them', async () => {
    const d = fakeD1();
    d.state.set('next_height', '1308');
    installGateway(1322, (h) => (h >= 1312 && h <= 1316 ? 'over' : 'ok'));
    const out = await syncDeployRegistry(mkEnv(d.db));
    expect(out.nextHeight).toBe(1323);
    expect(out.skippedHeights).toEqual([1312, 1313, 1314, 1315, 1316]);
    expect(d.state.get('next_height')).toBe('1323');
    expect(JSON.parse(d.state.get('skipped_blocks')!).map((x: { height: number }) => x.height)).toEqual([1312, 1313, 1314, 1315, 1316]);
    expect(JSON.parse(d.state.get('last_sync')!)).toMatchObject({ ok: true, skippedHeights: [1312, 1313, 1314, 1315, 1316] });
  });

  it('does not re-request skipped heights on the next tick and writes no cursor when idle', async () => {
    const d = fakeD1();
    d.state.set('next_height', '1308');
    const calls = installGateway(1322, (h) => (h >= 1312 && h <= 1316 ? 'over' : 'ok'));
    await syncDeployRegistry(mkEnv(d.db));
    const writesAfterFirst = d.stats.writes;
    calls.length = 0;
    const out = await syncDeployRegistry(mkEnv(d.db));
    expect(calls).toEqual([]); // at tip: no block requests at all
    expect(out.indexedBlocks).toBe(0);
    // only the last_sync telemetry row is written; cursor and skip list are untouched
    expect(d.stats.writes - writesAfterFirst).toBe(1);
  });

  it('holds the cursor at a transient failure and keeps earlier blocks', async () => {
    const d = fakeD1();
    d.state.set('next_height', '100');
    installGateway(110, (h) => (h === 103 ? 'http503' : 'ok'));
    const out = await syncDeployRegistry(mkEnv(d.db));
    expect(out.nextHeight).toBe(103);
    expect(d.state.get('next_height')).toBe('103');
    expect(d.state.get('skipped_blocks')).toBeUndefined();
    const t = JSON.parse(d.state.get('last_sync')!);
    expect(t.ok).toBe(false);
    expect(t.error).toMatch(/stopped at height 103/);
  });

  it('throws and leaves the cursor untouched when the very first height fails transiently', async () => {
    const d = fakeD1();
    d.state.set('next_height', '100');
    installGateway(110, (h) => (h === 100 ? 'timeout' : 'ok'));
    await expect(syncDeployRegistry(mkEnv(d.db))).rejects.toBeDefined();
    expect(d.state.get('next_height')).toBe('100');
    expect(d.state.get('skipped_blocks')).toBeUndefined();
    expect(JSON.parse(d.state.get('last_sync')!).ok).toBe(false);
  });
});

