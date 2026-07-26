#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BIN_NAME=kitsune
PROFILE=debug
BUILD=1
DRY_RUN=0
EXTRA_ARGS=()

usage() {
  cat <<'EOF'
usage: scripts/live_handoff_dev.sh [kitsune] [--bin kitsune] [--release] [--no-build] [--dry-run] [-- <live-handoff args>]

Build a repo-local Kitsune binary and live-handoff the currently targeted
server to that binary. This is meant to be run from inside an attached Kitsune
session while preserving the inherited KITSUNE_SOCKET_PATH.

Defaults:
  binary:  kitsune
  profile: debug

Examples:
  scripts/live_handoff_dev.sh
  scripts/live_handoff_dev.sh kitsune
  scripts/live_handoff_dev.sh --release --bin kitsune
  make handoff

Notes:
  - The script invokes target/<profile>/kitsune directly, not an installed CLI.
  - If KITSUNE_SOCKET_PATH is set, handoff targets that attached session.
  - If KITSUNE_SOCKET_PATH is unset, the CLI's normal default/session selection applies.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    kitsune)
      BIN_NAME=$1
      shift
      ;;
    --bin)
      if [[ $# -lt 2 ]]; then
        echo "error: --bin requires kitsune" >&2
        exit 2
      fi
      BIN_NAME=$2
      shift 2
      ;;
    --bin=*)
      BIN_NAME=${1#--bin=}
      shift
      ;;
    --release)
      PROFILE=release
      shift
      ;;
    --no-build)
      BUILD=0
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help | -h)
      usage
      exit 0
      ;;
    --)
      shift
      EXTRA_ARGS+=("$@")
      break
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$BIN_NAME" in
  kitsune)
    echo "selected binary: $BIN_NAME" >&2
    ;;
  *)
    echo "error: binary must be 'kitsune', got '$BIN_NAME'" >&2
    exit 2
    ;;
esac

case "$(uname -s)" in
  Darwin | Linux) ;;
  *)
    echo "error: live handoff is only supported on Unix-like hosts" >&2
    exit 1
    ;;
esac

if [[ $BUILD -eq 1 ]]; then
  build_args=(build --locked --bin "$BIN_NAME")
  if [[ "$PROFILE" == "release" ]]; then
    build_args+=(--release)
  fi
  echo "+ cargo ${build_args[*]}" >&2
  if [[ $DRY_RUN -eq 0 ]]; then
    cargo "${build_args[@]}"
  fi
fi

IMPORT_EXE="$ROOT_DIR/target/$PROFILE/$BIN_NAME"
if [[ $DRY_RUN -eq 0 && ! -x "$IMPORT_EXE" ]]; then
  echo "error: expected executable not found: $IMPORT_EXE" >&2
  echo "hint: rerun without --no-build or choose the matching --release/profile" >&2
  exit 1
fi

if [[ -n "${KITSUNE_SOCKET_PATH:-}" ]]; then
  echo "target socket: $KITSUNE_SOCKET_PATH" >&2
else
  echo "target socket: <default selected by $BIN_NAME>" >&2
fi

echo "+ $IMPORT_EXE server live-handoff --import-exe $IMPORT_EXE ${EXTRA_ARGS[*]}" >&2
if [[ $DRY_RUN -eq 1 ]]; then
  exit 0
fi
"$IMPORT_EXE" server live-handoff --import-exe "$IMPORT_EXE" "${EXTRA_ARGS[@]}"
