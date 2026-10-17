#!/bin/sh
# Fly / container entrypoint for boing-node.
# Flags come from env so the same image can run validator+faucet or a peering full node.
set -eu

DATA_DIR="${BOING_DATA_DIR:-/data}"
P2P_LISTEN="${BOING_P2P_LISTEN:-/ip4/0.0.0.0/tcp/4001}"
RPC_PORT="${BOING_RPC_PORT:-8545}"

mkdir -p "$DATA_DIR"

set -- /usr/local/bin/boing-node \
  --data-dir "$DATA_DIR" \
  --p2p-listen "$P2P_LISTEN" \
  --rpc-port "$RPC_PORT"

if [ "${BOING_VALIDATOR:-0}" = "1" ]; then
  set -- "$@" --validator
fi

if [ "${BOING_FAUCET_ENABLE:-0}" = "1" ]; then
  set -- "$@" --faucet-enable
fi

# BOING_BOOTNODES (secret) plus BOING_EXTRA_BOOTNODES (plain env, e.g. /dns6/<peer-app>.internal/tcp/4001
# so the peer's current private address is re-resolved on every redial).
BOOTNODES="${BOING_BOOTNODES:-}"
if [ -n "${BOING_EXTRA_BOOTNODES:-}" ]; then
  BOOTNODES="${BOOTNODES:+$BOOTNODES,}$BOING_EXTRA_BOOTNODES"
fi
if [ -n "$BOOTNODES" ]; then
  set -- "$@" --bootnodes "$BOOTNODES"
fi

if [ -n "${BOING_PENDING_TXS_PER_SENDER:-}" ]; then
  set -- "$@" --pending-txs-per-sender "$BOING_PENDING_TXS_PER_SENDER"
fi

echo "Starting boing-node validator=${BOING_VALIDATOR:-0} faucet=${BOING_FAUCET_ENABLE:-0} rpc=${RPC_PORT} data=${DATA_DIR}"
exec "$@"
