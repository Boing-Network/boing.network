#!/usr/bin/env bash
# Report whether Windows Authenticode inputs are present for Hub release CI.
# Does NOT invent, decode, or validate a .pfx — only checks non-empty env vars.
# Usage: ./desktop-hub/scripts/check-windows-signing-ready.sh
# Exit 0 = all three set; 1 = missing (unsigned expected); 2 = unexpected error.
set -euo pipefail

vars=(
  WINDOWS_CERTIFICATE
  WINDOWS_CERTIFICATE_PASSWORD
  WINDOWS_CERTIFICATE_THUMBPRINT
)

missing=0
echo "Windows Authenticode readiness (Boing Network Hub)"
echo "--------------------------------------------------"
for v in "${vars[@]}"; do
  # ${!v-} expands empty when unset without tripping set -u.
  val="${!v-}"
  if [[ -n "${val}" ]]; then
    # Never print secret contents — only presence + length.
    echo "  ${v}: SET (length ${#val})"
  else
    echo "  ${v}: MISSING"
    missing=$((missing + 1))
  fi
done
echo "--------------------------------------------------"

if [[ "${missing}" -eq 0 ]]; then
  echo "STATUS: ready — release workflow can attempt Authenticode signing"
  exit 0
fi

echo "STATUS: not ready — ${missing}/3 secret(s) missing; Windows Hub builds stay unsigned"
echo "Docs: docs/WINDOWS-AUTHENTICODE.md"
exit 1
