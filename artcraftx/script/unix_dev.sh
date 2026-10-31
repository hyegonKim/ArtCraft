#!/usr/bin/env bash
# Start the ArtCraftX frontend and Tauri app together on macOS or Linux.
set -euo pipefail

root_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
frontend_path="${root_dir}/frontend"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<'HELP'
Usage: ./script/unix_dev.sh

Starts Vite with JavaScript/CSS live reload and Tauri with Rust rebuild/restart.
Searches for a free loopback port starting at ARTCRAFTX_DEV_PORT (default: 5183).
Rust uses Tauri IPC; it does not need a second HTTP port.
Ctrl-C stops this launcher's frontend, Rust watcher, and desktop app.

Requires Node.js 20+, npm, Rust, cargo-tauri 2, and Tauri platform prerequisites.
Installs frontend dependencies if node_modules is missing.
HELP
  exit 0
fi

if [[ $# -ne 0 ]]; then
  echo "Unknown argument: $1 (see --help)" >&2
  exit 1
fi

source "${root_dir}/script/common/frontend_preflight.sh"
frontend_preflight "${frontend_path}"

if ! command -v cargo >/dev/null || ! cargo tauri --version >/dev/null 2>&1; then
  echo 'ERROR: Install Rust and the Tauri CLI: cargo install tauri-cli --version "^2" --locked' >&2
  exit 1
fi

cd "${frontend_path}"
if [[ ! -f node_modules/vite/package.json ]]; then
  npm install
fi

exec node "${frontend_path}/scripts/unix-dev.mjs"
