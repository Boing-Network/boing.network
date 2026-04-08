/**
 * Shared helpers for tutorial deploy scripts: deferred process.exit (Windows libuv UV_HANDLE_CLOSING)
 * and consistent JSON-RPC failure messages.
 */
import { explainBoingRpcError } from 'boing-sdk';

export class ScheduledExitError extends Error {
  constructor() {
    super('scheduled_exit');
    this.name = 'ScheduledExitError';
  }
}

/** Avoid Windows libuv UV_HANDLE_CLOSING when exiting right after RPC I/O. */
export function scheduleExit(code) {
  setTimeout(() => process.exit(code), 10);
}

export function exitAfterLog(code) {
  scheduleExit(code);
  throw new ScheduledExitError();
}

const rpcHint =
  'Non-2xx (e.g. HTTP 530 / error code 1033) usually means Cloudflare Tunnel origin is down. See docs/RUNBOOK.md § 8.3. Try BOING_RPC_URL=http://127.0.0.1:8545 with a local boing-node. On Windows, deferred process.exit avoids UV_HANDLE_CLOSING races after RPC I/O.';

/**
 * @param {{ getAccount: (hex: string) => Promise<{ nonce: string }> }} client
 * @param {string} rpc
 * @param {string} senderHex
 */
export async function getAccountScheduled(client, rpc, senderHex) {
  try {
    return await client.getAccount(senderHex);
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          rpc,
          phase: 'boing_getAccount',
          error: explainBoingRpcError(e),
          hint: rpcHint,
        },
        null,
        2
      )
    );
    exitAfterLog(1);
  }
}
