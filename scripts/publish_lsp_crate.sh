#!/bin/bash
set -eo pipefail
META_VERSION=$(cat VERSION)
CRATE_VERSION=$(toml get --toml-path autoschematic-lsp/Cargo.toml package.version)

python scripts/version_check.py $META_VERSION $CRATE_VERSION

cargo publish -p autoschematic-core 