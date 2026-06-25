#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=dev_common.sh
source "${ROOT}/scripts/dev_common.sh"

exec cargo +"${RUST_TOOLCHAIN:-stable}" run -p server --bin kaisha-server
