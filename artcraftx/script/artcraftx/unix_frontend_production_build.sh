#!/usr/bin/env bash
# This works on Linux and MacOS to run a production frontend build

root_dir=$(pwd)
frontend_path="${root_dir}/frontend"

echo "Running ArtCraftX Frontend Build..."
echo ""

pushd "${frontend_path}" || exit

npm install --verbose

export VITE_ENVIRONMENT_TYPE="production"

nx build artcraft

popd || exit
