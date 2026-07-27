#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/sign_dev_binary.sh <binary>" >&2
  exit 2
fi

binary=$1

case "$(uname -s)" in
  Darwin) ;;
  *) exit 0 ;;
esac

if [[ "${KITSUNE_SKIP_CODESIGN:-}" == "1" ]]; then
  echo "skipping macOS ad-hoc signing for $binary (KITSUNE_SKIP_CODESIGN=1)" >&2
  exit 0
fi

if [[ ! -f "$binary" ]]; then
  echo "error: cannot sign missing binary: $binary" >&2
  exit 1
fi

if ! command -v codesign >/dev/null 2>&1; then
  echo "error: codesign is required on macOS to prepare dev binary: $binary" >&2
  exit 1
fi

echo "+ codesign --force --sign - $binary" >&2
codesign --force --sign - "$binary" >/dev/null
