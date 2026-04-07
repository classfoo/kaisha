#!/usr/bin/env bash
set -euo pipefail

WEB_HOST="${KAISHA_WEB_HOST:-0.0.0.0}"
WEB_PORT="1420"
API_HOST="${KAISHA_HOST:-0.0.0.0}"
API_PORT="${KAISHA_PORT:-8080}"

KAISHA_HOST="${API_HOST}" \
KAISHA_PORT="${API_PORT}" \
npx concurrently -n WEB,API \
  "npm --workspace @kaisha/web run dev -- --host ${WEB_HOST} --port ${WEB_PORT}" \
  "npm run dev:server"
