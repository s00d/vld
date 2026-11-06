#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"

if [[ -z "${VERSION}" ]]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

bash scripts/release-preflight.sh
bash scripts/release-changelog.sh "${VERSION}"
# fuzz/ is outside the workspace: refresh its lock for the bumped version,
# or `cargo check --locked` in CI fails after every release.
cargo update --manifest-path fuzz/Cargo.toml --workspace --offline
