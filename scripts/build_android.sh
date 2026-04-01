#!/usr/bin/env bash
set -euo pipefail

MANIFEST_PATH="apps/desktop/src-tauri/Cargo.toml"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

npm run build:web
cargo +"$RUST_TOOLCHAIN" tauri android build --manifest-path "$MANIFEST_PATH"
