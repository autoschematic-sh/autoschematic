#!/bin/bash
set -exo pipefail
protoc \
    --plugin=protoc-gen-ts_proto=node_modules/.bin/protoc-gen-ts_proto \
    --ts_proto_out=./src/generated \
    --ts_proto_opt=outputServices=grpc-js,esModuleInterop=true \
    -I ../autoschematic-core/proto/ connector.proto
