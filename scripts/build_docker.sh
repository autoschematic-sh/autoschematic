#!/bin/bash
set -euxo pipefail
META_VERSION=$(cat VERSION)

CRATE_VERSION=$(toml get --toml-path autoschematic-server/Cargo.toml package.version)

python scripts/version_check.py $META_VERSION $CRATE_VERSION
DOCKER_BUILDKIT=1 docker build -t autoschematicsh/autoschematic:$CRATE_VERSION -f autoschematic-server/Dockerfile . 