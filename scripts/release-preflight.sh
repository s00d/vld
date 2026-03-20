#!/usr/bin/env bash
set -euo pipefail

CRATE_NAME="${1:-}"
VERSION="${2:-}"
DRY_RUN_FLAG="${3:-${DRY_RUN:-false}}"

if [[ -z "${CRATE_NAME}" || -z "${VERSION}" ]]; then
  echo "usage: $0 <crate-name> <version>" >&2
  exit 1
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Release preflight for ${CRATE_NAME} v${VERSION}"
echo "==> Dry run: ${DRY_RUN_FLAG}"

echo "==> Build (${CRATE_NAME}, all features)"
cargo build -p "${CRATE_NAME}" --all-features

echo "==> Test (${CRATE_NAME}, default features)"
cargo test -p "${CRATE_NAME}"

echo "==> Test (${CRATE_NAME}, all features)"
cargo test -p "${CRATE_NAME}" --all-features

echo "==> Clippy (${CRATE_NAME}, all targets, all features)"
cargo clippy -p "${CRATE_NAME}" --all-targets --all-features -- -D warnings

if [[ "${CRATE_NAME}" == "vld" ]]; then
  echo "==> vld feature matrix"
  cargo test -p vld --no-default-features
  cargo test -p vld --no-default-features --features serialize
  cargo test -p vld --no-default-features --features openapi
  cargo test -p vld --no-default-features --features diff
  cargo test -p vld --no-default-features --features "serialize,openapi,diff"
fi

if [[ "${DRY_RUN_FLAG}" == "true" ]]; then
  echo "==> Skip CHANGELOG update in dry-run"
else
  echo "==> Update CHANGELOG.md"
  git-cliff --config cliff.toml --tag "v${VERSION}" --output CHANGELOG.md
fi

echo "==> Release preflight completed"
