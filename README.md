# Kaisha

Kaisha is a Rust-first desktop and web application scaffold based on Tauri 2.x.  
It supports two runtime modes:

- Desktop integrated mode (Tauri app + embedded backend server)
- Backend-only mode (Rust server + browser frontend)

## Project Layout

```text
.
├── apps
│   ├── desktop
│   │   └── src-tauri          # Tauri host application (desktop shell)
│   └── web                    # React + Vite frontend
├── crates
│   ├── domain                 # Core business models and domain types
│   ├── application            # Use-cases and application services
│   └── server                 # HTTP API adapter (Axum), standalone server binary
├── configs
│   └── runtime.env.example    # Runtime environment template
├── scripts                    # Dev/build/init scripts for all targets
├── Cargo.toml                 # Rust workspace root
└── package.json               # Node workspace scripts
```

## Architecture (Layered / Tier-I)

### 1) Domain Layer (`crates/domain`)

- Holds pure business data structures and domain concepts.
- No framework or UI dependency.
- Should remain stable and reusable across all delivery channels.

### 2) Application Layer (`crates/application`)

- Implements use-cases and orchestration logic.
- Depends on `domain`.
- No direct dependency on HTTP, Tauri, or frontend frameworks.

### 3) Delivery Layer

- `crates/server`: HTTP API delivery (Axum), exposes endpoints such as `/api/health`.
- `apps/desktop/src-tauri`: desktop runtime host, starts local backend and opens the app window.
- `apps/web`: web UI client, can run in browser-only mode and call the backend API.

## Runtime Modes

### Desktop Integrated Mode

Use a single command to start Tauri desktop app:

```bash
npm run dev:desktop
```

Flow:

1. Frontend dev server starts (Vite).
2. Tauri host launches.
3. Embedded backend service runs locally.
4. UI communicates with local API.

### Backend-Only Browser Mode

Run backend + web frontend in parallel:

```bash
npm run dev:browser
```

Flow:

1. Rust server runs as standalone process.
2. Frontend runs in browser.
3. Frontend accesses API via `VITE_API_BASE`.

## Development Guide

### 1) First-Time Setup

1. Clone repository and enter project root:
   ```bash
   git clone <your-repo-url>
   cd kaisha
   ```
2. Run one-shot environment initialization:
   ```bash
   npm run init:env
   ```
3. If you need mobile development dependencies:
   ```bash
   npm run init:env:mobile
   ```
4. Confirm local checks:
   ```bash
   npm run check:web
   cargo check --workspace
   ```

### 2) Start Development

Choose one runtime mode:

- Desktop app development (Tauri + local backend):
  ```bash
  npm run dev:desktop
  ```
- Browser development (web + standalone backend):
  ```bash
  npm run dev:browser
  ```
- API-only development:
  ```bash
  npm run dev:server
  ```

### 3) Typical Feature Workflow

1. Define or update domain models in `crates/domain`.
2. Implement use-case orchestration in `crates/application`.
3. Expose API endpoints in `crates/server`.
4. Integrate frontend behavior in `apps/web`.
5. Validate both paths:
   - browser mode (`npm run dev:browser`)
   - desktop mode (`npm run dev:desktop`)

### 4) Daily Validation Commands

```bash
npm run check:web
cargo check --workspace
```

Use these before committing to reduce integration issues.

## Build and Packaging

### Common Build Commands

```bash
npm run build:web
npm run build:server
npm run build:desktop
```

You can also run a one-shot build chain:

```bash
bash ./scripts/build_all.sh
```

### Multi-Platform Packaging

```bash
npm run build:linux
npm run build:macos
npm run build:windows
npm run build:ios
npm run build:android
```

## Build Guide (Step-by-Step)

### A) Build Browser + Backend Artifacts

1. Build web static files:
   ```bash
   npm run build:web
   ```
   Output: `apps/web/dist`
2. Build backend binary:
   ```bash
   npm run build:server
   ```
   Output: `target/release/kaisha-server`

### B) Build Desktop Artifact (Current Host)

1. Ensure web build is available (handled automatically by Tauri config).
2. Run:
   ```bash
   npm run build:desktop
   ```
3. Check generated packages under `target/release/bundle`.

### C) Build Platform-Specific Desktop Packages

- Linux:
  ```bash
  npm run build:linux
  ```
- macOS:
  ```bash
  npm run build:macos
  ```
- Windows:
  ```bash
  npm run build:windows
  ```

### D) Build Mobile Packages

1. Initialize platform project once:
   ```bash
   npm run init:ios
   npm run init:android
   ```
2. Build mobile targets:
   ```bash
   npm run build:ios
   npm run build:android
   ```

### E) Auto-Select Desktop Build by Host OS

```bash
npm run build:platform
```

Or explicitly request mobile target:

```bash
bash ./scripts/build_all_platforms.sh ios
bash ./scripts/build_all_platforms.sh android
```

## Troubleshooting

### 1) `cargo` / `rustup` Not Found

Symptoms:

- `cargo: command not found`
- Rust-related build commands fail immediately

Fix:

```bash
npm run init:env
source "$HOME/.cargo/env"
rustup show
```

If shell still cannot find Rust binaries, ensure `$HOME/.cargo/bin` is in your `PATH`.

### 2) `cargo tauri` Not Found

Symptoms:

- `error: no such command: tauri`

Fix:

```bash
cargo install tauri-cli
cargo tauri --help
```

### 3) Linux Desktop Build Fails (WebKitGTK / GTK Missing)

Symptoms:

- Linker errors for `webkit2gtk`, `gtk`, `appindicator`, or `rsvg`

Fix (Ubuntu/Debian):

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config \
  libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf
```

Then retry:

```bash
npm run build:linux
```

### 4) Frontend Build Fails (`npm` / node modules issues)

Symptoms:

- Missing package errors
- TypeScript/Vite dependency resolution failures

Fix:

```bash
rm -rf node_modules package-lock.json
npm install
npm run check:web
```

### 5) Port Already In Use (`8080` or `1420`)

Symptoms:

- Dev server fails to start with address/port in use

Fix:

1. Change runtime ports in environment variables.
2. Example:
   ```bash
   KAISHA_PORT=18080 VITE_API_BASE=http://127.0.0.1:18080 npm run dev:browser
   ```

### 6) macOS iOS Build Issues (Xcode / Signing)

Symptoms:

- iOS build/signing errors
- Simulator/device deployment fails

Fix:

```bash
xcode-select --install
sudo xcodebuild -license accept
```

Also verify:

- Xcode is installed and opened at least once
- valid Apple developer team/signing profile is configured

Then run:

```bash
npm run init:ios
npm run build:ios
```

### 7) Android Build Issues (SDK / NDK / Licenses)

Symptoms:

- Android target/toolchain not found
- Gradle reports missing SDK components

Fix:

1. Install Android Studio and required SDK/NDK via SDK Manager.
2. Set environment variables (`ANDROID_HOME` or `ANDROID_SDK_ROOT`).
3. Accept SDK licenses.

Then run:

```bash
npm run init:android
npm run build:android
```

### 8) Windows Packaging Issues (MSVC / NSIS)

Symptoms:

- linker errors mentioning MSVC build tools
- NSIS/MSI bundle creation fails

Fix:

1. Install Visual Studio C++ Build Tools (Desktop development with C++).
2. Ensure environment is MSVC-compatible (`x86_64-pc-windows-msvc` target).
3. Re-run:
   ```bash
   npm run build:windows
   ```

### 9) `npm run init:env` Fails Midway

Symptoms:

- Package manager not available (`apt` / `brew` / `choco`)
- permission-denied during install

Fix:

1. Re-run with preview first:
   ```bash
   bash ./scripts/init_env.sh --yes --dry-run
   ```
2. Install missing package manager manually.
3. Re-run full initialization:
   ```bash
   npm run init:env
   ```

### Platform-Aware Entry

Auto-detect host desktop platform:

```bash
npm run build:platform
```

Or pass mobile target explicitly:

```bash
bash ./scripts/build_all_platforms.sh ios
bash ./scripts/build_all_platforms.sh android
```

### Mobile Initialization (first-time setup)

```bash
npm run init:ios
npm run init:android
```

## Environment Configuration

Copy and adjust values from:

- `configs/runtime.env.example`

Key variables:

- `KAISHA_HOST`: backend bind host (default `127.0.0.1`)
- `KAISHA_PORT`: backend bind port (default `8080`)
- `VITE_API_BASE`: frontend API base URL

## Development Notes

- Rust code is organized as a workspace, so backend/domain evolution remains modular.
- Frontend and desktop packaging are decoupled from core business logic.
- Add new business capabilities by extending:
  - `domain` for core types/rules
  - `application` for use-cases
  - `server` for API routes/adapters
  - `web` for UI integration
