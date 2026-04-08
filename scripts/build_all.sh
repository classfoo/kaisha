#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

cd "$PROJECT_ROOT"
npm install
npm run build:web
cargo +"$RUST_TOOLCHAIN" build -p server --bin kaisha-server --release

cd "$PROJECT_ROOT/apps/desktop/src-tauri"
cargo +"$RUST_TOOLCHAIN" tauri build
