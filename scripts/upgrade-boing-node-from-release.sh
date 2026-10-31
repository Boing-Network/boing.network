#!/usr/bin/env bash
# Replace target/release/boing-node (and optional boing CLI) from an official GitHub release zip.
# Run on the **tunnel origin** (the machine where boing-node listens on the port cloudflared forwards to).
#
# Prerequisite: stop the running boing-node (Ctrl+C, systemd, or Task Manager) so the binary is not locked.
# The Cloudflare tunnel does **not** ship binaries; this script only updates files on this host.
#
# Usage:
#   chmod +x scripts/upgrade-boing-node-from-release.sh
#   ./scripts/upgrade-boing-node-from-release.sh
#   ./scripts/upgrade-boing-node-from-release.sh testnet-v0.1.22   # other tag: pass expected SHA via env (see below)
#
# Optional env:
#   BOING_EXPECT_SHA256   — required if TAG is not built into the script’s pin table
#   BOING_WITH_CLI=1      — also install `boing` next to `boing-node`
#   BOING_FORCE=1         — skip “Press Enter” prompt

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

TAG="${1:-testnet-v0.1.21}"

# Built-in SHA256 of release zips (refresh when you cut a new tag; or use BOING_EXPECT_SHA256).
LINUX_ZIP_SHA=""
MAC_ZIP_SHA=""
case "$TAG" in
  testnet-v0.1.21)
    LINUX_ZIP_SHA="ecf3dd80d9e52c1b73387c7c437eda11ea470f6fc149e98a2fa7d7f7e5eaecca"
    MAC_ZIP_SHA="e11e4301eee6edaa96983a0122a5b095e6cfe5d51e77d04a4279a0d8825ad1d5"
    ;;
  testnet-v0.1.8)
    LINUX_ZIP_SHA="70355e6e6c6c9f33804957df1c215a531bec0c329fe5c1fc48f3d23350bd296c"
    MAC_ZIP_SHA="435216299129a6bcc04d4775cf7956315246c4860bf2fd8a769df93bea7e7bbc"
    ;;
esac

detect_platform() {
  case "$(uname -sm)" in
    Linux\ x86_64) echo linux ;;
    Darwin\ arm64) echo macos ;;
    *)
      echo "This shell script supports Linux x86_64 and macOS arm64 only." >&2
      echo "On Windows, run: powershell -File scripts/upgrade-boing-node-from-release.ps1" >&2
      exit 1
      ;;
  esac
}

PLATFORM="$(detect_platform)"

ZIP_NAME=""
NODE_NAME_IN_ZIP=""
CLI_NAME_IN_ZIP=""
EXPECT_SHA=""

if [[ "$PLATFORM" == "linux" ]]; then
  ZIP_NAME="release-linux-x86_64.zip"
  NODE_NAME_IN_ZIP="boing-node-linux-x86_64"
  CLI_NAME_IN_ZIP="boing-linux-x86_64"
  EXPECT_SHA="${BOING_EXPECT_SHA256:-$LINUX_ZIP_SHA}"
elif [[ "$PLATFORM" == "macos" ]]; then
  ZIP_NAME="release-macos-aarch64.zip"
  NODE_NAME_IN_ZIP="boing-node-macos-aarch64"
  CLI_NAME_IN_ZIP="boing-macos-aarch64"
  EXPECT_SHA="${BOING_EXPECT_SHA256:-$MAC_ZIP_SHA}"
fi

if [[ -z "$EXPECT_SHA" ]]; then
  echo "No built-in SHA256 for tag '$TAG' on $PLATFORM. Set BOING_EXPECT_SHA256 to the zip file’s sha256sum." >&2
  exit 1
fi

URL="https://github.com/Boing-Network/boing.network/releases/download/${TAG}/${ZIP_NAME}"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

if [[ -z "${BOING_FORCE:-}" ]]; then
  echo "This will replace $REPO_ROOT/target/release/boing-node (tag: $TAG, zip: $ZIP_NAME)."
  echo "Stop boing-node first. Press Enter to continue or Ctrl+C to abort."
  read -r _
fi

echo "Downloading $URL ..."
curl -fsSL -o "$STAGE/$ZIP_NAME" "$URL"

echo "Verifying SHA256 ..."
if command -v openssl >/dev/null 2>&1; then
  ACTUAL="$(openssl dgst -sha256 -r "$STAGE/$ZIP_NAME" | awk '{print $1}')"
else
  ACTUAL="$(sha256sum "$STAGE/$ZIP_NAME" | awk '{print $1}')"
fi
norm() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }
if [[ "$(norm "$ACTUAL")" != "$(norm "$EXPECT_SHA")" ]]; then
  echo "SHA256 mismatch for $ZIP_NAME" >&2
  echo "  expected: $EXPECT_SHA" >&2
  echo "  actual:   $ACTUAL" >&2
  exit 1
fi

unzip -q -o "$STAGE/$ZIP_NAME" -d "$STAGE/extract"
NODE_SRC="$STAGE/extract/$NODE_NAME_IN_ZIP"
if [[ ! -f "$NODE_SRC" ]]; then
  echo "Zip did not contain $NODE_NAME_IN_ZIP" >&2
  exit 1
fi

mkdir -p "$REPO_ROOT/target/release"
DEST="$REPO_ROOT/target/release/boing-node"
if [[ -f "$DEST" ]]; then
  cp -a "$DEST" "${DEST}.bak.$(date +%Y%m%d%H%M%S)"
fi
cp -a "$NODE_SRC" "$DEST"
chmod +x "$DEST"
echo "Installed: $DEST"

if [[ "${BOING_WITH_CLI:-}" == "1" ]]; then
  CLI_SRC="$STAGE/extract/$CLI_NAME_IN_ZIP"
  if [[ ! -f "$CLI_SRC" ]]; then
    echo "Zip did not contain $CLI_NAME_IN_ZIP (skip CLI)" >&2
  else
    CLI_DEST="$REPO_ROOT/target/release/boing"
    [[ -f "$CLI_DEST" ]] && cp -a "$CLI_DEST" "${CLI_DEST}.bak.$(date +%Y%m%d%H%M%S)"
    cp -a "$CLI_SRC" "$CLI_DEST"
    chmod +x "$CLI_DEST"
    echo "Installed: $CLI_DEST"
  fi
fi

echo "Done. Restart boing-node (e.g. ./scripts/start-bootnode-1.sh). cloudflared usually needs no change if the RPC port stays 8545."
