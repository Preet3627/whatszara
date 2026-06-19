#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────
# Whatszara — One-command setup for non-technical users
# Zero Python required. Just Go + Rust + Node.
# ──────────────────────────────────────────────

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
pass()  { echo -e "${GREEN}[PASS]${NC}  $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
fail()  { echo -e "${RED}[FAIL]${NC}  $1"; exit 1; }

OS="$(uname -s)"
ARCH="$(uname -m)"
info "Detected: ${OS} (${ARCH})"

# ── Help ──────────────────────────────────────
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  echo ""
  echo "  Whatszara Setup Script"
  echo ""
  echo "  Usage:"
  echo "    ./setup.sh            Full setup (default)"
  echo "    ./setup.sh check      Just check prerequisites"
  echo "    ./setup.sh bridge     Start WhatsApp bridge after setup"
  echo ""
  exit 0
fi

PREREQ_MODE=false
[[ "${1:-}" == "check" ]] && PREREQ_MODE=true

check_cmd() {
  if command -v "$1" &>/dev/null; then
    pass "$1 found: $(command -v "$1")"
    return 0
  else
    warn "$1 not found"
    return 1
  fi
}

NEED_INSTALL=()

check_prereqs() {
  info "Checking prerequisites..."
  check_cmd go           || NEED_INSTALL+=("go")
  check_cmd node         || NEED_INSTALL+=("node")
  check_cmd cargo        || NEED_INSTALL+=("cargo (Rust)")
  check_cmd npm          || NEED_INSTALL+=("npm")

  if command -v ffmpeg &>/dev/null; then
    pass "ffmpeg found"
  else
    warn "ffmpeg not found (optional — needed for audio messages)"
  fi

  if [[ "$PREREQ_MODE" == true ]]; then
    if [[ ${#NEED_INSTALL[@]} -eq 0 ]]; then
      pass "All prerequisites satisfied!"
    else
      warn "Missing: ${NEED_INSTALL[*]}"
      echo "  Run ./setup.sh to auto-install."
    fi
    exit 0
  fi
}

install_homebrew() {
  if ! command -v brew &>/dev/null; then
    info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  else
    pass "Homebrew found"
  fi
}

install_deps_macos() {
  info "Installing via Homebrew..."
  install_homebrew
  brew update
  command -v go &>/dev/null     || brew install go
  command -v node &>/dev/null   || brew install node
  command -v cargo &>/dev/null  || brew install rust
  command -v ffmpeg &>/dev/null || brew install ffmpeg
}

install_deps_linux() {
  info "Installing via apt..."
  sudo apt-get update -qq
  sudo apt-get install -y -qq golang-go nodejs cargo ffmpeg curl 2>/dev/null || true
  command -v cargo &>/dev/null || {
    warn "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
  }
}

install_deps_windows() {
  echo ""
  warn "Windows: Install manually:"
  echo "  - Go:    https://go.dev/dl/"
  echo "  - Node:  https://nodejs.org/"
  echo "  - Rust:  https://rustup.rs/"
  echo "  Then run this script again."
}

setup_go_bridge() {
  info "Building Go WhatsApp bridge..."
  cd whatsapp-bridge
  go mod download 2>/dev/null || true
  go build -o ../desktop-app/src-tauri/bin/whatsapp-bridge-$OS .
  cd ..
  pass "Go bridge built"
}

setup_desktop_app() {
  info "Setting up Tauri desktop app..."
  cd desktop-app
  npm install 2>/dev/null
  cd ..
  pass "Desktop app ready"
}

# ── Main ──────────────────────────────────────
main() {
  echo ""
  echo "╔══════════════════════════════════════════╗"
  echo "║        Whatszara — One-Click Setup       ║"
  echo "╚══════════════════════════════════════════╝"
  echo ""

  check_prereqs

  if [[ ${#NEED_INSTALL[@]} -gt 0 ]]; then
    info "Installing ${#NEED_INSTALL[@]} missing dependencies..."
    case "$OS" in
      Darwin) install_deps_macos ;;
      Linux)  install_deps_linux ;;
      *)      install_deps_windows ;;
    esac
  else
    pass "All dependencies already installed!"
  fi

  setup_go_bridge
  setup_desktop_app

  echo ""
  echo "╔══════════════════════════════════════════╗"
  echo "║           Setup Complete! 🎉             ║"
  echo "╚══════════════════════════════════════════╝"
  echo ""
  echo "  Quick Start:"
  echo ""
  echo "  1. Start WhatsApp bridge:"
  echo "     \$ cd whatsapp-bridge && go run main.go"
  echo "     (Scan QR code with WhatsApp mobile app)"
  echo ""
  echo "  2. Launch desktop app:"
  echo "     \$ cd desktop-app && npm run tauri dev"
  echo ""
  echo "  Or: make bridge   (WhatsApp bridge)"
  echo "      make desktop  (Desktop app)"
  echo ""
  echo "  No Python required. One binary, one install."
  echo ""
  echo "  Need help? https://github.com/Preet3627/whatszara"
  echo ""

  if [[ "${1:-}" == "bridge" ]]; then
    info "Starting WhatsApp bridge..."
    cd whatsapp-bridge && go run main.go
  fi
}

main "$@"
