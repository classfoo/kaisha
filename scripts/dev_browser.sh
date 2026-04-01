#!/usr/bin/env bash
set -euo pipefail

WEB_HOST="${CODEBAND_WEB_HOST:-0.0.0.0}"
WEB_PORT="1420"
API_HOST="${CODEBAND_HOST:-0.0.0.0}"
API_PORT="${CODEBAND_PORT:-8080}"

CODEBAND_HOST="${API_HOST}" \
CODEBAND_PORT="${API_PORT}" \
npx concurrently -n WEB,API \
  "npm --workspace @codeband/web run dev -- --host ${WEB_HOST} --port ${WEB_PORT}" \
  "npm run dev:server"
