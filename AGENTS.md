# AGENTS.md

## Cursor Cloud specific instructions

### Services (browser E2E)

| Service | Port | Command |
|---------|------|---------|
| `kaisha-server` (API) | 8080 | `npm run dev:server` |
| Vite (`@kaisha/web`) | 1420 | `npm --workspace @kaisha/web run dev` |
| Both | 8080 + 1420 | `bash ./scripts/dev_browser.sh` or `npm run dev:browser` |

Use a **tmux** session for `dev:browser` / `dev_browser.sh` so the stack stays up across agent turns. `scripts/dev_browser.sh` binds **0.0.0.0** (not only localhost), which matters in remote/cloud VMs.

Smoke check: `curl http://127.0.0.1:8080/api/health` → `{"status":"ok"}`.

### Validation commands

See root `package.json` and `README.md` for the canonical list:

- `npm run check:web` — TypeScript (`tsc --noEmit`)
- `npm run check:rust` — `cargo check --workspace` (includes Tauri desktop crate)
- `cargo test -p server` — backend unit/integration tests

**Gotcha:** On a clean tree, `npm run check:rust` can fail on `kaisha-desktop` until `apps/web/dist` exists. Run `npm run build:web` once before the first full-workspace check (or use `cargo check -p domain -p application -p server` for API-only work).

**Linux desktop/Tauri:** Requires GTK/WebKit dev packages (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, etc.). See README “Linux Desktop Build Fails”.

### Workspace and API keys

- Default workspace falls back to `~/.kaisha` when unset; configure via UI or `POST /api/workspace` with `{"path":"<dir>"}`.
- AI tool CLIs and provider API keys are optional for UI shell smoke tests; required for hire/chat/autonomy flows (stored under workspace `settings/tools/`).

### Tests note

As of setup verification, `cargo test -p server` reports **132 passed, 1 failed** (`work_task::tests::filter_by_biz_and_assignee`). Treat as a known upstream failure when interpreting CI-like runs in Cloud Agent VMs.
