#!/bin/bash
set -exo pipefail

OUT_DIR=./autoschematic_sdk/generated

python -m grpc_tools.protoc \
    --python_out="$OUT_DIR" \
    --pyi_out="$OUT_DIR" \
    --grpc_python_out="$OUT_DIR" \
    -I ../autoschematic-core/proto/ \
    connector.proto

# Fix imports: protoc generates absolute imports (import connector_pb2)
# but we need relative imports (from . import connector_pb2) for package use.
sed -i 's/^import connector_pb2 as/from . import connector_pb2 as/' "$OUT_DIR/connector_pb2_grpc.py"

# grpc.experimental is needed at runtime but not always auto-imported
sed -i '/^import grpc$/a import grpc.experimental' "$OUT_DIR/connector_pb2_grpc.py"