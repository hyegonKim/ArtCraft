#!/usr/bin/env bash
# This works on Linux and MacOS to build production ArtCraftX

set -euxo pipefail

echo "Building production ArtCraftX..."
echo ""

root_dir=$(pwd)
frontend_path="${root_dir}/frontend"
rust_crate_path="${root_dir}/crates/artcraftx"

config_path="${rust_crate_path}/tauri.conf.json"

pushd "${frontend_path}" || exit

npm install --verbose

popd || exit

export TAURI_FRONTEND_PATH="${frontend_path}"
export TAURI_APP_PATH="${rust_crate_path}"

# Put SQLx into offline mode (no DB hits / migrations).
export SQLX_OFFLINE=true

# NB: The "frontend dev" script sets "production" too, so this must only control the
# hostnames we use, not minification, etc.
export VITE_ENVIRONMENT_TYPE="production"

# This appears to trigger "nx build" instead of "nx dev".
cargo tauri build --config "${config_path}"

echo "Done!"

date "+Finished on %A, %B %e - %H:%M:%S (local timezone)"
