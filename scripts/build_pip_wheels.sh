#!/bin/bash
#
# Build a platform-specific Python wheel containing the autoschematic binary.
# Also includes autoschematic-lsp.
#
# Usage:
#   ./scripts/build_pip_wheels.sh <rust-target> <path-to-binary>
#
# Example:
#   ./scripts/build_pip_wheels.sh x86_64-unknown-linux-gnu target/release/autoschematic
#
set -euo pipefail

RUST_TARGET="${1:?Usage: $0 <rust-target> <path-to-binary>}"
BINARY_PATH="${2:?Usage: $0 <rust-target> <path-to-binary>}"

VERSION="$(cat VERSION)"
DIST_DIR="dist"

# Map Rust target triples to Python platform tags.
case "$RUST_TARGET" in
    x86_64-unknown-linux-gnu)
        PLATFORM_TAG="manylinux_2_17_x86_64.manylinux2014_x86_64"
        ;;
    aarch64-unknown-linux-gnu)
        PLATFORM_TAG="manylinux_2_17_aarch64.manylinux2014_aarch64"
        ;;
    x86_64-apple-darwin)
        PLATFORM_TAG="macosx_10_12_x86_64"
        ;;
    aarch64-apple-darwin)
        PLATFORM_TAG="macosx_11_0_arm64"
        ;;
    x86_64-pc-windows-msvc)
        PLATFORM_TAG="win_amd64"
        ;;
    *)
        echo "Error: unsupported target '$RUST_TARGET'" >&2
        exit 1
        ;;
esac

WHEEL_NAME="autoschematic-${VERSION}-py3-none-${PLATFORM_TAG}.whl"
DATA_DIR="autoschematic-${VERSION}.data"
DIST_INFO="autoschematic-${VERSION}.dist-info"

TMPDIR="$(mktemp -d)"
# trap 'rm -rf "$TMPDIR"' EXIT

# --- Binary in data/scripts ---
mkdir -p "$TMPDIR/${DATA_DIR}/scripts"
cp "$BINARY_PATH" "$TMPDIR/${DATA_DIR}/scripts/autoschematic"
chmod +x "$TMPDIR/${DATA_DIR}/scripts/autoschematic"

cp "$BINARY_PATH"-lsp "$TMPDIR/${DATA_DIR}/scripts/autoschematic-lsp"
chmod +x "$TMPDIR/${DATA_DIR}/scripts/autoschematic-lsp"

# --- dist-info ---
mkdir -p "$TMPDIR/${DIST_INFO}"

cat > "$TMPDIR/${DIST_INFO}/METADATA" <<EOF
Metadata-Version: 2.1
Name: autoschematic
Version: ${VERSION}
Summary: Command-line tools for Autoschematic
Home-page: https://autoschematic.sh
Author: Peter Sherman
Author-email: peter@autoschematic.sh
License: AGPL-3.0
Project-URL: Repository, https://github.com/autoschematic-sh/autoschematic
Requires-Python: >=3.8
EOF

cat > "$TMPDIR/${DIST_INFO}/WHEEL" <<EOF
Wheel-Version: 1.0
Generator: autoschematic-build
Root-Is-Purelib: false
Tag: py3-none-${PLATFORM_TAG}
EOF

# --- RECORD (hashes computed after packing; leave empty for the RECORD entry itself) ---
cd "$TMPDIR"

# Compute sha256 hashes for RECORD
python3 -c "
import hashlib, base64, os

entries = []
for root, dirs, files in os.walk('.'):
    for f in files:
        path = os.path.join(root, f)[2:]  # strip leading ./
        if path == '${DIST_INFO}/RECORD':
            continue
        with open(path, 'rb') as fh:
            digest = hashlib.sha256(fh.read()).digest()
        b64 = base64.urlsafe_b64encode(digest).rstrip(b'=').decode()
        size = os.path.getsize(path)
        entries.append(f'{path},sha256={b64},{size}')

entries.append('${DIST_INFO}/RECORD,,')

with open('${DIST_INFO}/RECORD', 'w') as f:
    f.write('\n'.join(entries) + '\n')
"

# --- Pack into wheel (a zip file) ---
mkdir -p "${OLDPWD}/${DIST_DIR}"
zip -r "${OLDPWD}/${DIST_DIR}/${WHEEL_NAME}" . -x '.*'

cd "${OLDPWD}"
echo "Built: ${DIST_DIR}/${WHEEL_NAME}"