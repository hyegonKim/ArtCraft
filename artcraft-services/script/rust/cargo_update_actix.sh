#!/bin/bash

set -euxo pipefail

echo 'Upgrading actix dependencies in Cargo.toml files'
# Error: dependencies actix-codec, actix-macros, actix-multipart-derive, actix-router, actix-server don't exist
cargo upgrade \
  --verbose \
  -p actix \
  -p actix-files \
  -p actix-http \
  -p actix-multipart \
  -p actix-rt \
  -p actix-web \
  -p actix-web-actors \
  -p actix-web-lab

echo 'Updating actix dependencies in Cargo.lock'

# TODO: actix-cors update was causing issues
cargo update \
  --verbose \
  actix \
  actix-codec \
  actix-files \
  actix-http \
  actix-macros \
  actix-multipart \
  actix-multipart-derive \
  actix-router \
  actix-rt \
  actix-server \
  actix-web \
  actix-web-actors \
  actix-web-lab
