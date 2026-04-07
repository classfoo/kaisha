#!/usr/bin/env bash
set -euo pipefail

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

npm install
npm run build:web
cargo +"$RUST_TOOLCHAIN" build -p server --bin kaisha-server --release
cargo +"$RUST_TOOLCHAIN" tauri build --manifest-path apps/desktop/src-tauri/Cargo.toml
