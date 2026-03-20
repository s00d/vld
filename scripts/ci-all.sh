#!/usr/bin/env bash
set -euo pipefail

# Full local CI run (future-proof for new workspace crates).
#
# Why this stays up to date:
# - workspace-wide build/test/clippy/fmt automatically includes new crates
# - explicit feature-matrix checks remain only for core `vld`

echo "==> Build workspace (all features)"
cargo build --workspace --all-features

echo "==> Test workspace (default features)"
cargo test --workspace

echo "==> Test workspace (all features)"
cargo test --workspace --all-features

echo "==> Test vld feature matrix"
cargo test -p vld --no-default-features
cargo test -p vld --no-default-features --features serialize
cargo test -p vld --no-default-features --features openapi
cargo test -p vld --no-default-features --features diff
cargo test -p vld --no-default-features --features "serialize,openapi,diff"

echo "==> Clippy"
cargo clippy --workspace --all-features -- -D warnings

echo "==> Format check"
cargo fmt --all --check

echo "==> Playground"
cargo run -p playground

echo "==> CI all checks passed"
