#!/usr/bin/env bash
# Launch the native Rust Relapse executor UI.
# Relapse profile sockets are discovered automatically under /tmp.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
export PATH="$HOME/.cargo/bin:$PATH"

BIN="$HERE/target/release/relapse-ui"
# Rebuild when sources are newer than the binary so a launch never runs a stale build.
if [[ ! -x "$BIN" ]] || [[ -n "$(find "$HERE/src" "$HERE/Cargo.toml" "$HERE/style.css" -newer "$BIN" -print -quit 2>/dev/null)" ]]; then
  cargo build --release --manifest-path "$HERE/Cargo.toml"
fi

exec "$BIN" "$@"
