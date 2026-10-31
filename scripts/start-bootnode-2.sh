#!/bin/bash
# Bootnode 2 — Secondary machine. Connects to Bootnode 1.
#
# 1. Set BOOTNODE_1_IP to your primary machine's public IP (run "curl -s ifconfig.me" on primary).
# 2. Ensure firewall allows TCP 4001 inbound on this machine.
# 3. After running, share this machine's public IP for PUBLIC_BOOTNODES.

BOOTNODE_1_IP="${BOOTNODE_1_IP:-169.155.48.188}"
set -e
cd "$(dirname "$0")/.."

if [ ! -f target/release/boing-node ]; then
  echo "Building boing-node..."
  cargo build --release
fi

echo "Starting Bootnode 2 (validator)..."
echo "P2P: 0.0.0.0:4001"
echo "Connecting to Bootnode 1: /ip4/$BOOTNODE_1_IP/tcp/4001"
echo "RPC: http://127.0.0.1:8546"
echo ""
echo "Your public IP: curl -s ifconfig.me"
echo ""

./target/release/boing-node \
  --p2p-listen /ip4/0.0.0.0/tcp/4001 \
  --bootnodes "/ip4/$BOOTNODE_1_IP/tcp/4001" \
  --validator \
  --rpc-port 8546 \
  --data-dir ./bootnode2-data
