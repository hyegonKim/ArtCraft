#!/bin/bash

set -euxo pipefail

echo 'Updating and checking hakari (dependency graph build optimization)'

cargo hakari generate
cargo hakari manage-deps
cargo hakari verify

# TODO: Ask for user input before applying some automated heuristics

echo 'Done.'
