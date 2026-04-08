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
npm run build:web

cd "$PROJECT_ROOT/apps/desktop/src-tauri"

# 如果 Xcode 项目目录不存在，先初始化
if [[ ! -d "gen/apple" ]]; then
  echo "Initializing iOS project..."
  cargo +"$RUST_TOOLCHAIN" tauri ios init
fi

cargo +"$RUST_TOOLCHAIN" tauri ios build
