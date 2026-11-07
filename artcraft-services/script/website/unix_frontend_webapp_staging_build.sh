#!/usr/bin/env bash
set -euo pipefail

# Rebuild dependencies in production mode after tests. A direct `vite build`
# can consume test-mode library dist files containing incompatible jsxDEV calls.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
services_root="$(cd -- "$script_dir/../.." && pwd)"
frontend_root="$services_root/frontend"
output_zip="${1:-${TMPDIR:-/tmp}/artcraft-private-staging.zip}"
source_commit="${ARTCRAFT_BUILD_COMMIT:-$(git -C "$services_root" rev-parse HEAD)}"

: "${VITE_ARTCRAFT_API_ORIGIN:?Set the HTTPS staging API origin}"
: "${VITE_GOOGLE_CLIENT_ID:?Set the staging Google web client ID}"

cd "$frontend_root"
NODE_ENV=production NX_DAEMON=false NX_ISOLATE_PLUGINS=false \
  VITE_ARTCRAFT_LOCAL_EDITING=false \
  node_modules/.bin/nx build @frontend/artcraft-webapp --skipNxCache

python3 - "$frontend_root/apps/artcraft-webapp" "$output_zip" "$source_commit" <<'PY'
import datetime
import json
import pathlib
import re
import shutil
import sys
import zipfile

app, output, commit = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
if not re.fullmatch(r"[0-9a-f]{40}", commit):
  raise SystemExit("A full source commit SHA is required")
dist = app / "dist"
scripts = list((dist / "assets").glob("*.js"))
if not scripts:
  raise SystemExit("No production JavaScript was built")
for script in scripts:
  if re.search(r"\.jsxDEV\s*\(", script.read_text()):
    raise SystemExit(f"Development JSX in {script.name}; refusing to package")
for asset in ("jsm/libs/ammo.wasm.js", "jsm/libs/ammo.wasm.wasm"):
  if not (dist / asset).is_file():
    raise SystemExit(f"Missing runtime asset: {asset}")
for name in ("_headers", "_redirects", "_staging-unavailable.html"):
  shutil.copy2(app / "staging" / name, dist / name)
(dist / "artcraft-build.json").write_text(json.dumps({
  "commit": commit,
  "local_video_editing": False,
  "private_cloud_storage": True,
  "production_build": True,
  "build_command": "nx build @frontend/artcraft-webapp",
  "built_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
}, indent=2))
output = output.resolve()
output.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
  for file in sorted(dist.rglob("*")):
    if file.is_file():
      archive.write(file, file.relative_to(dist))
print(f"Verified production staging ZIP: {output}")
print(f"Source commit: {commit}")
PY
