#!/bin/bash
set -eo pipefail
META_VERSION=$(cat VERSION)
CRATE_VERSION=$(toml get --toml-path autoschematic/Cargo.toml package.version)
python scripts/version_check.py $META_VERSION $CRATE_VERSION
# TODO version should just be the autoschematic server version!
DOCKER_BUILDKIT=1 docker build -t autoschematicsh/autoschematic:$CRATE_VERSION .