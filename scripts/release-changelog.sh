#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"

if [[ -z "${VERSION}" ]]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

if [[ "${DRY_RUN:-false}" == "true" ]]; then
  echo "==> Skip CHANGELOG update in dry-run"
  exit 0
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Update CHANGELOG.md"
git cliff --config cliff.toml --tag "v${VERSION}" --output CHANGELOG.md
