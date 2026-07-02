#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
DRY_RUN_FLAG="${DRY_RUN:-false}"

if [[ -z "${VERSION}" ]]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Release preflight for workspace v${VERSION}"
echo "==> Dry run: ${DRY_RUN_FLAG}"

bash scripts/ci-all.sh

if [[ "${DRY_RUN_FLAG}" == "true" ]]; then
  echo "==> Skip CHANGELOG update in dry-run"
else
  echo "==> Update CHANGELOG.md"
  git-cliff --config cliff.toml --tag "v${VERSION}" --output CHANGELOG.md
fi

echo "==> Release preflight completed"
