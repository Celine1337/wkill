#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release

# extension.js syntax check (ESM), if node is available
if command -v node >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.mjs)"
  cp extension/extension.js "$tmp"
  node --check "$tmp"
  rm -f "$tmp"
  echo "extension.js syntax OK"
fi

BIN=./target/release/wkill

# live smoke test: only when the bridge extension is active
if "$BIN" list >/dev/null 2>&1; then
  if command -v zenity >/dev/null 2>&1; then
    zenity --info --title="wkill-test" --text="wkill live test" >/dev/null 2>&1 &
    ZPID=$!
    for _ in $(seq 1 40); do
      "$BIN" list | grep -q wkill-test && break
      sleep 0.25
    done
    if ! "$BIN" list | grep -q wkill-test; then
      kill "$ZPID" 2>/dev/null || true
      echo "FAIL: test window never appeared"
      exit 1
    fi
    echo 1 | "$BIN" pick wkill-test
    for _ in $(seq 1 20); do
      kill -0 "$ZPID" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$ZPID" 2>/dev/null; then
      kill -9 "$ZPID" 2>/dev/null || true
      echo "FAIL: test window survived"
      exit 1
    fi
    echo "live smoke OK (sacrificial zenity window killed)"
  else
    echo "live smoke: zenity not found, skipped kill test"
  fi
else
  echo "live smoke skipped: bridge not active (run wkill install, log out/in once)"
fi
echo "ALL CHECKS PASSED"