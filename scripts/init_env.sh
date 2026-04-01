#!/usr/bin/env bash
set -euo pipefail

# One-shot environment bootstrap for Codeband.
# Rust is managed only via rustup (no Homebrew-based Rust install).

AUTO_YES=0
DRY_RUN=0
WITH_MOBILE=0
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes|-y)
      AUTO_YES=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --with-mobile)
      WITH_MOBILE=1
      shift
      ;;
    --rust-toolchain)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --rust-toolchain"
        exit 1
      fi
      RUST_TOOLCHAIN="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1"
      echo "Usage: bash ./scripts/init_env.sh [--yes] [--dry-run] [--with-mobile] [--rust-toolchain <version|channel>]"
      exit 1
      ;;
  esac
done

log() { echo "[init] $*"; }
warn() { echo "[init][warn] $*"; }

run_cmd() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $*"
  else
    eval "$@"
  fi
}

ask() {
  local message="$1"
  if [[ "$AUTO_YES" -eq 1 ]]; then
    return 0
  fi
  read -r -p "$message [y/N]: " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}

has_cmd() {
  command -v "$1" >/dev/null 2>&1
}

can_use_sudo_noninteractive() {
  has_cmd sudo && sudo -n true >/dev/null 2>&1
}

OS="$(uname -s)"

ensure_node_with_nvm_unix() {
  if has_cmd node && has_cmd npm; then
    log "Node.js already installed"
    return
  fi

  if [[ "$OS" != "Linux" && "$OS" != "Darwin" ]]; then
    return
  fi

  run_cmd "export PROFILE=/dev/null && curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash"

  if [[ "$DRY_RUN" -eq 0 ]]; then
    export NVM_DIR="$HOME/.nvm"
    # shellcheck disable=SC1091
    source "$NVM_DIR/nvm.sh"
  fi

  run_cmd "export NVM_DIR=\"$HOME/.nvm\" && [ -s \"$HOME/.nvm/nvm.sh\" ] && . \"$HOME/.nvm/nvm.sh\" && nvm install --lts && nvm use --lts"
}

install_linux_apt() {
  if ! can_use_sudo_noninteractive; then
    warn "sudo non-interactive access unavailable; skipping apt dependency auto-install."
    warn "Run manually (interactive shell): sudo apt-get update && sudo apt-get install -y curl git build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf"
    return 1
  fi

  run_cmd "sudo apt-get update"
  run_cmd "sudo apt-get install -y curl git build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf"

  if ! has_cmd node || ! has_cmd npm; then
    run_cmd "sudo apt-get install -y nodejs npm"
  fi

  if [[ "$WITH_MOBILE" -eq 1 ]]; then
    run_cmd "sudo apt-get install -y openjdk-17-jdk"
    warn "Android SDK/NDK are not auto-installed here. Install Android Studio, then configure ANDROID_HOME and sdkmanager packages."
  fi
}

install_macos_base() {
  if ! has_cmd xcode-select; then
    warn "xcode-select is missing; install Xcode Command Line Tools manually."
    return 1
  fi

  run_cmd "xcode-select --install || true"

  if ! has_cmd node || ! has_cmd npm; then
    ensure_node_with_nvm_unix
  fi

  if [[ "$WITH_MOBILE" -eq 1 ]]; then
    warn "Install Android Studio manually for Android SDK/NDK setup."
    warn "Install full Xcode from App Store for iOS packaging and signing."
  fi
}

install_windows_choco() {
  if ! has_cmd choco; then
    warn "Chocolatey is missing. Install from https://chocolatey.org/install first."
    return 1
  fi

  run_cmd "choco install -y git curl nodejs"

  if [[ "$WITH_MOBILE" -eq 1 ]]; then
    run_cmd "choco install -y openjdk17 androidstudio"
    warn "Install Android SDK/NDK from Android Studio SDK Manager after launch."
  fi
}

ensure_nodejs() {
  if has_cmd node && has_cmd npm; then
    log "Node.js already installed"
    return
  fi

  case "$OS" in
    Linux)
      install_linux_apt || true
      ;;
    Darwin)
      ensure_node_with_nvm_unix
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      install_windows_choco || true
      ;;
    *)
      warn "Unable to auto-install Node.js for OS: $OS"
      ;;
  esac
}

ensure_rustup() {
  if has_cmd rustup; then
    log "rustup already installed"
    return
  fi

  case "$OS" in
    Linux|Darwin)
      run_cmd "curl https://sh.rustup.rs -sSf | sh -s -- -y"
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      if has_cmd choco; then
        run_cmd "choco install -y rustup.install"
      else
        warn "Install rustup manually from https://rustup.rs"
      fi
      ;;
    *)
      warn "Unsupported OS for rustup auto-install: $OS"
      ;;
  esac
}

load_cargo_env() {
  if [[ "$DRY_RUN" -eq 0 && -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi
}

ensure_rust_toolchain() {
  run_cmd "rustup toolchain install $RUST_TOOLCHAIN"
  run_cmd "rustup update $RUST_TOOLCHAIN"
  run_cmd "rustup default $RUST_TOOLCHAIN"
  run_cmd "rustup component add rustfmt clippy --toolchain $RUST_TOOLCHAIN"
  log "Rust toolchain set to: $RUST_TOOLCHAIN"
}

ensure_tauri_cli() {
  if has_cmd cargo-tauri; then
    log "cargo-tauri already installed"
  else
    run_cmd "cargo install tauri-cli --locked"
  fi
}

ensure_node_modules() {
  run_cmd "npm install"
}

ensure_rust_targets() {
  run_cmd "rustup target add x86_64-unknown-linux-gnu --toolchain $RUST_TOOLCHAIN || true"
  run_cmd "rustup target add x86_64-apple-darwin aarch64-apple-darwin --toolchain $RUST_TOOLCHAIN || true"
  run_cmd "rustup target add x86_64-pc-windows-msvc --toolchain $RUST_TOOLCHAIN || true"

  if [[ "$WITH_MOBILE" -eq 1 ]]; then
    run_cmd "rustup target add aarch64-apple-ios aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android --toolchain $RUST_TOOLCHAIN || true"
  fi
}

verify_project() {
  run_cmd "npm run check:web"
  run_cmd "cargo +$RUST_TOOLCHAIN check -p domain -p application -p server"
}

main() {
  log "Detected OS: $OS"
  log "Target Rust toolchain: $RUST_TOOLCHAIN"

  if ask "Proceed with environment bootstrap?"; then
    case "$OS" in
      Linux)
        install_linux_apt || true
        ;;
      Darwin)
        install_macos_base || true
        ;;
      MINGW*|MSYS*|CYGWIN*|Windows_NT)
        install_windows_choco || true
        ;;
      *)
        warn "Unsupported OS for automatic package manager install: $OS"
        ;;
    esac

    ensure_rustup
    load_cargo_env
    ensure_rust_toolchain
    ensure_nodejs
    ensure_tauri_cli
    ensure_node_modules
    ensure_rust_targets
    verify_project

    log "Environment initialization completed."
  else
    log "Initialization cancelled by user."
  fi
}

main
