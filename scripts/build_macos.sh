#!/usr/bin/env bash
set -euo pipefail

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

MANIFEST_PATH="$PROJECT_ROOT/apps/desktop/src-tauri/Cargo.toml"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

npm run build:web
cargo +"$RUST_TOOLCHAIN" tauri build --bundles app,dmg -- --manifest-path "$MANIFEST_PATH"
