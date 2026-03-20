#!/usr/bin/env bash
set -euo pipefail

# Full local CI run (future-proof for new workspace crates).
#
# Why this stays up to date:
# - workspace-wide build/test/clippy/fmt automatically includes new crates
# - explicit feature-matrix checks remain only for core `vld`

VLD_EXTENDED_FEATURES="chrono,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"

echo "==> Build workspace (default features)"
cargo build --workspace

echo "==> Test workspace (default features)"
cargo test --workspace

echo "==> Test vld feature matrix"
cargo test -p vld --no-default-features
cargo test -p vld --no-default-features --features serialize
cargo test -p vld --no-default-features --features openapi
cargo test -p vld --no-default-features --features diff
cargo test -p vld --no-default-features --features "serialize,openapi,diff"
cargo test -p vld --features "${VLD_EXTENDED_FEATURES}"

echo "==> Clippy (workspace, default features)"
cargo clippy --workspace -- -D warnings

echo "==> Clippy (vld extended features)"
cargo clippy -p vld --all-targets --features "${VLD_EXTENDED_FEATURES}" -- -D warnings

echo "==> Format check"
cargo fmt --all --check

echo "==> Playground"
cargo run -p playground

echo "==> CI all checks passed"
