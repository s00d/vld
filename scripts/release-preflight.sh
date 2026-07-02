#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Release preflight (workspace CI)"
bash scripts/ci-all.sh
echo "==> Release preflight completed"
