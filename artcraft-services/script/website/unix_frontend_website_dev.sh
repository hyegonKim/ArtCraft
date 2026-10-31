#!/usr/bin/env bash
# This works on Linux and MacOS to launch the website (Next.js) dev server

root_dir=$(pwd)
frontend_path="${root_dir}/frontend"
website_path="${frontend_path}/apps/artcraft-website-next"
port=3000

source "${root_dir}/script/common/frontend_preflight.sh"
frontend_preflight "${frontend_path}"

echo "Running Artcraft Website (Next.js) in Dev Mode..."
echo ""

# Kill any process running on the dev port, which will block startup
if lsof -i tcp:${port} &>/dev/null; then
  lsof -i tcp:${port} -t | xargs kill -9
  echo "Killed process running on port ${port}"
else
  echo "No process running on port ${port}"
fi

# The Next.js app is excluded from the frontend npm workspace and has its own
# package-lock.json, so it installs and runs from its own directory.
pushd "${website_path}" || exit

frontend_npm_install

# Backend devs: launch with USE_LOCAL_API=1 to point the website at a local
# storyteller-web on http://localhost:12345. Otherwise it hits production.
if [[ "${USE_LOCAL_API:-0}" == "1" ]]; then
  export NEXT_PUBLIC_API_HOST="http://localhost:12345"
  echo "USE_LOCAL_API=1 — API calls will target ${NEXT_PUBLIC_API_HOST}"
else
  echo "API calls will target production (set USE_LOCAL_API=1 for a local backend)"
fi

npm run dev -- --port ${port}

popd || exit
