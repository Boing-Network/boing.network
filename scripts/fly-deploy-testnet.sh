#!/usr/bin/env bash
# Create and deploy the two-node Fly.io Boing testnet (validator + peering full node).
# Requires: fly CLI authenticated (`fly auth whoami`), repo root as cwd.
set -euo pipefail
# Git Bash on Windows rewrites /ip4/... as a filesystem path unless this is set.
export MSYS_NO_PATHCONV=1

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ORG="${FLY_ORG:-personal}"
REGION="${FLY_REGION:-iad}"
APP1="${FLY_APP_1:-boing-testnet-1}"
APP2="${FLY_APP_2:-boing-testnet-2}"
VOLUME_NAME="${FLY_VOLUME_NAME:-boing_data}"
VOLUME_SIZE="${FLY_VOLUME_SIZE:-10}"
CONFIG1="fly.testnet-1.toml"
CONFIG2="fly.testnet-2.toml"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

need fly
need openssl

echo "Fly identity: $(fly auth whoami)"
echo "Org=${ORG} region=${REGION} apps=${APP1},${APP2}"

ensure_app() {
  local app="$1"
  if fly status -a "$app" >/dev/null 2>&1; then
    echo "app exists: $app"
  else
    echo "creating app: $app"
    fly apps create "$app" --org "$ORG"
  fi
}

ensure_volume() {
  local app="$1"
  if fly volumes list -a "$app" --json 2>/dev/null | grep -q "\"name\": \"${VOLUME_NAME}\""; then
    echo "volume exists: ${VOLUME_NAME} on $app"
  else
    echo "creating volume ${VOLUME_NAME} (${VOLUME_SIZE}GB) on $app"
    fly volumes create "$VOLUME_NAME" --region "$REGION" --size "$VOLUME_SIZE" -a "$app" -y
  fi
}

ensure_ips() {
  local app="$1"
  echo "allocating IPs for $app (shared v4 for HTTPS; dedicated v4 for public P2P if the org allows it)"
  fly ips allocate-v4 --shared -a "$app" >/dev/null 2>&1 || true
  fly ips allocate-v6 -a "$app" >/dev/null 2>&1 || true
  if ! fly ips list -a "$app" | awk 'NR>1 && $1=="v4" && $3!="shared" {found=1} END{exit !found}'; then
    fly ips allocate-v4 -a "$app" || echo "warning: dedicated IPv4 not allocated for $app (public P2P bootnodes may be IPv6-only)"
  fi
}

ensure_operator_secret() {
  local app="$1"
  if fly secrets list -a "$app" | awk 'NR>1 {print $1}' | grep -qx "BOING_OPERATOR_RPC_TOKEN"; then
    echo "secret already set: BOING_OPERATOR_RPC_TOKEN on $app"
    return
  fi
  if [ -z "${BOING_OPERATOR_RPC_TOKEN:-}" ]; then
    BOING_OPERATOR_RPC_TOKEN="$(openssl rand -hex 24)"
    echo "generated BOING_OPERATOR_RPC_TOKEN (set on both apps; not printed)"
    export BOING_OPERATOR_RPC_TOKEN
  fi
  fly secrets set "BOING_OPERATOR_RPC_TOKEN=${BOING_OPERATOR_RPC_TOKEN}" -a "$app" --stage
}

public_p2p_multiaddrs() {
  local app="$1"
  local addrs=""
  local ip
  while read -r ip; do
    [ -n "$ip" ] || continue
    addrs="${addrs},/ip4/${ip}/tcp/4001"
  done < <(fly ips list -a "$app" --json 2>/dev/null | python -c "
import json,sys
try:
    data=json.load(sys.stdin)
except Exception:
    sys.exit(0)
items=data if isinstance(data,list) else data.get('ips', data.get('Addresses', []))
for item in items or []:
    t=str(item.get('type') or item.get('Type') or '')
    ip=str(item.get('address') or item.get('Address') or '')
    region=str(item.get('region') or item.get('Region') or '')
    if t.lower()=='v4' and ip and region.lower()!='global' and '/' not in ip:
        print(ip)
")
  while read -r ip; do
    [ -n "$ip" ] || continue
    addrs="${addrs},/ip6/${ip}/tcp/4001"
  done < <(fly ips private -a "$app" --json 2>/dev/null | python -c "
import json,sys
try:
    data=json.load(sys.stdin)
except Exception:
    sys.exit(0)
items=data if isinstance(data,list) else data.get('addresses', [])
for item in items or []:
    ip=str(item.get('ip') or item.get('IP') or item.get('address') or '')
    if ip:
        print(ip.split('/')[0].strip('[]'))
")
  echo "${addrs#,}"
}

ensure_app "$APP1"
ensure_app "$APP2"
ensure_volume "$APP1"
ensure_volume "$APP2"
ensure_ips "$APP1"
ensure_ips "$APP2"
ensure_operator_secret "$APP1"
ensure_operator_secret "$APP2"

echo "deploying $APP1 (this builds the Rust node on Fly's remote builder)"
fly deploy --config "$CONFIG1" --remote-only --ha=false

BOOTNODES="$(public_p2p_multiaddrs "$APP1")"
if [ -n "$BOOTNODES" ]; then
  echo "wiring $APP2 bootnodes: $BOOTNODES"
  fly secrets set "BOING_BOOTNODES=${BOOTNODES}" -a "$APP2" --stage
else
  echo "warning: could not discover $APP1 P2P addresses; $APP2 will start without bootnodes"
fi

echo "deploying $APP2 (reuse image from $APP1 when possible)"
IMAGE="$(fly image show -a "$APP1" --json 2>/dev/null | python -c "
import json,sys
try:
    data=json.load(sys.stdin)
except Exception:
    sys.exit(0)
if isinstance(data, list) and data:
    print(data[0].get('ref') or data[0].get('digest') or '')
elif isinstance(data, dict):
    print(data.get('ref') or '')
" || true)"

if [ -n "${IMAGE:-}" ]; then
  fly deploy --config "$CONFIG2" --remote-only --ha=false --image "$IMAGE" || \
    fly deploy --config "$CONFIG2" --remote-only --ha=false
else
  fly deploy --config "$CONFIG2" --remote-only --ha=false
fi

BOOT2="$(public_p2p_multiaddrs "$APP2")"
if [ -n "$BOOT2" ]; then
  echo "adding $APP2 as a bootnode on $APP1"
  COMBINED="${BOOTNODES:+$BOOTNODES,}$BOOT2"
  fly secrets set "BOING_BOOTNODES=${COMBINED}" -a "$APP1"
fi

echo
echo "Deployed."
echo "  RPC 1: https://${APP1}.fly.dev/"
echo "  RPC 2: https://${APP2}.fly.dev/"
echo "  Health: curl -fsS https://${APP1}.fly.dev/live"
echo "  Probe:  curl -fsS -A boing-sdk/json-rpc -X POST https://${APP1}.fly.dev/ -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"boing_health\",\"params\":[]}'"
echo
echo "This is a hosted Fly testnet (fresh chain on first volume). It does not automatically replace https://testnet-rpc.boing.network/"
