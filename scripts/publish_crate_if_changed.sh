set -euo pipefail

CRATE="${1:?Usage: $0 <crate-name> <cargo-token>}"
TOKEN="${2:?Usage: $0 <crate-name> <cargo-token>}"

if [ -n "$(git diff --name-only origin/main -- $CRATE/Cargo.toml)" ]; then
  cargo publish --package $CRATE --token $TOKEN
else
  echo "$CRATE/Cargo.toml did not change; skipping publish"
fi
