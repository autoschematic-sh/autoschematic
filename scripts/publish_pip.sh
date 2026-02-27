#!/bin/bash
#
# Publish all built wheels to PyPI.
#
# Usage:
#   ./scripts/publish_pip.sh
#
# Requires TWINE_USERNAME and TWINE_PASSWORD (or TWINE_API_KEY) to be set,
# or a ~/.pypirc file.
#
set -euo pipefail

DIST_DIR="dist"

if ! command -v twine &>/dev/null; then
    echo "Error: twine is not installed. Run: pip install twine" >&2
    exit 1
fi

if [ -z "$(ls -A "${DIST_DIR}"/*.whl 2>/dev/null)" ]; then
    echo "Error: no wheels found in ${DIST_DIR}/" >&2
    exit 1
fi

echo "Uploading wheels to PyPI:"
ls "${DIST_DIR}"/*.whl

twine upload "${DIST_DIR}"/*.whl