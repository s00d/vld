#!/usr/bin/env bash
# Release preflight: the CI groups that check what gets published. msrv,
# minimal and playground are left to CI (msrv/minimal resolve their own
# lockfiles; they need extra toolchains).
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Release preflight (workspace CI)"
VLD_CI_ALLOW_DIRTY=1 bash scripts/ci-all.sh test features clippy docs fmt package
echo "==> Release preflight completed"
