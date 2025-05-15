#!/bin/bash
set -eo pipefail
META_VERSION=$(cat VERSION)
# TODO version should just be the autoschematic server version!
docker run -i -t -v `pwd`/autoschematic/secret:/app/secret --network host --env-file autoschematic/.dockerenv autoschematicsh/autoschematic:$META_VERSION