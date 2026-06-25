#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=dev_common.sh
source "$(dirname "$0")/dev_common.sh"

# Tauri dev expects Vite on localhost:1420. Clean up stale listeners
# (for example from an interrupted prior session) before starting.
EXISTING_PIDS="$(lsof -tiTCP:1420 -sTCP:LISTEN || true)"
if [[ -n "${EXISTING_PIDS}" ]]; then
  echo "Port 1420 is busy; stopping existing process(es): ${EXISTING_PIDS}"
  kill ${EXISTING_PIDS} || true
  sleep 1
fi

npm run dev:desktop
