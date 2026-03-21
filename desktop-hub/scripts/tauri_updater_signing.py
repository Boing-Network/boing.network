#!/usr/bin/env python3
"""
Boing Network Hub - Tauri updater signing helper.

Generates a minisign keypair (via the Tauri CLI), suggests a strong password,
and prints the exact GitHub Actions secrets to add.

Usage (from repo root):
  python desktop-hub/scripts/tauri_updater_signing.py generate
  python desktop-hub/scripts/tauri_updater_signing.py generate --password "your-phrase"
  python desktop-hub/scripts/tauri_updater_signing.py instructions

Requires: Node/npm (for npx @tauri-apps/cli@2). On Windows, use Git Bash or
PowerShell; unset CI if `npx tauri signer` complains about --ci.
"""

from __future__ import annotations

import argparse
import os
import secrets
import string
import subprocess
import sys
from pathlib import Path


def hub_root() -> Path:
    """desktop-hub/ (parent of scripts/)."""
    return Path(__file__).resolve().parent.parent


def default_key_path() -> Path:
    return hub_root() / ".tauri" / "boing-hub.key"


def generate_password(length: int) -> str:
    alphabet = string.ascii_letters + string.digits
    return "".join(secrets.choice(alphabet) for _ in range(length))


def run_signer_generate(key_path: Path, password: str) -> None:
    key_path.parent.mkdir(parents=True, exist_ok=True)
    hub = hub_root()
    cmd = [
        "npx",
        "@tauri-apps/cli@2",
        "signer",
        "generate",
        "-w",
        str(key_path),
        "-f",
        "-p",
        password,
    ]
    env = os.environ.copy()
    env.pop("CI", None)  # avoid CLI treating CI=1 as non-interactive error
    print("Running:", " ".join(cmd[:6]), "...", "(password hidden)\n")
    subprocess.run(cmd, cwd=str(hub), env=env, check=True)


def cmd_generate(args: argparse.Namespace) -> int:
    key_path = Path(args.output).expanduser()
    if not key_path.is_absolute():
        key_path = (hub_root() / key_path).resolve()

    if args.password is not None:
        password = args.password
        print("Using the password you passed with --password.\n")
    else:
        password = generate_password(args.length)
        print("Generated password (use for GitHub secret TAURI_SIGNING_PRIVATE_KEY_PASSWORD):\n")
        print("  " + password)
        print("\nStore it in a password manager; it cannot be recovered from the key file.\n")

    run_signer_generate(key_path, password)
    pub_path = key_path.with_suffix(".key.pub")
    if not pub_path.is_file():
        print(f"Expected public key at {pub_path} - check CLI output.", file=sys.stderr)
        return 1

    pub_text = pub_path.read_text(encoding="utf-8").strip()
    print("Public key file:", pub_path)
    print("\n--- Paste this into desktop-hub/src-tauri/tauri.conf.json > plugins.updater.pubkey ---\n")
    print(pub_text)
    print("\n--- End pubkey ---\n")

    priv_text = key_path.read_text(encoding="utf-8")
    if "untrusted comment" not in priv_text.splitlines()[0]:
        print("Warning: private key first line should start with 'untrusted comment'.", file=sys.stderr)

    cmd_instructions_after_generate(key_path=key_path, password_was_generated=args.password is None)
    return 0


def cmd_instructions(_args: argparse.Namespace) -> int:
    cmd_instructions_after_generate(key_path=None, password_was_generated=False)
    return 0


def cmd_instructions_after_generate(*, key_path: Path | None, password_was_generated: bool) -> None:
    print("=" * 72)
    print("GitHub Actions - repository secrets (Settings > Secrets and variables > Actions)")
    print("=" * 72)
    print("""
1) TAURI_SIGNING_PRIVATE_KEY
   - Open your PRIVATE key file in a text editor (two lines).
   - Line 1: untrusted comment: ...
   - Line 2: long base64 string
   - Copy BOTH lines exactly (including newline between them) into the secret.

2) TAURI_SIGNING_PRIVATE_KEY_PASSWORD
   - Required only if you generated the key WITH a password (this script does).
   - Paste the same password you used (or the generated one printed above).
   - If you use an EMPTY password (tauri signer ... -p ""), do NOT create this secret.
""")
    if key_path is not None:
        print(f"Private key file to copy from: {key_path}\n")
    if password_was_generated:
        print("You generated a random password above - add it as TAURI_SIGNING_PRIVATE_KEY_PASSWORD.\n")
    print("After updating tauri.conf.json pubkey, commit and push, then cut a new desktop-hub/v* tag.")
    print("=" * 72)


def cmd_show_paths(_args: argparse.Namespace) -> int:
    k = default_key_path()
    print("Default private key path:", k)
    print("Default public key path: ", k.with_suffix(".key.pub"))
    print("Exists:", k.is_file())
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="Tauri updater signing helper for Boing Network Hub")
    sub = p.add_subparsers(dest="cmd", required=True)

    g = sub.add_parser("generate", help="Generate minisign keypair via Tauri CLI (with password)")
    g.add_argument(
        "-o",
        "--output",
        default=".tauri/boing-hub.key",
        help="Private key path relative to desktop-hub/ (default: .tauri/boing-hub.key)",
    )
    g.add_argument(
        "-p",
        "--password",
        default=None,
        help="Password for the key. If omitted, a random password is generated and printed.",
    )
    g.add_argument(
        "-l",
        "--length",
        type=int,
        default=24,
        help="Length of generated password (default: 24)",
    )
    g.set_defaults(func=cmd_generate)

    i = sub.add_parser("instructions", help="Print GitHub secrets checklist only")
    i.set_defaults(func=cmd_instructions)

    s = sub.add_parser("paths", help="Show default key paths")
    s.set_defaults(func=cmd_show_paths)

    args = p.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
