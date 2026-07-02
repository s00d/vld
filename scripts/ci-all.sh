#!/usr/bin/env bash
set -euo pipefail

# Full local CI run (future-proof for new workspace crates).
#
# Why this stays up to date:
# - workspace-wide build/test/clippy with `--all-targets` covers tests/examples/benches
# - workspace-wide `--all-features` covers optional integrations without manual crate lists
# - explicit feature-matrix checks remain only for core `vld`

VLD_EXTENDED_FEATURES="chrono,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"
JIFF_FEATURES="jiff,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"
TIME_FEATURES="time,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"

echo "==> Build workspace (default features, all targets)"
cargo build --workspace --all-targets

echo "==> Test workspace (default features, all targets)"
cargo test --workspace --all-targets

echo "==> Test integration crates: legacy/new major branches"
cargo check -p vld-sqlx --no-default-features --features "sqlx-0_8,sqlite"
cargo check -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite"
cargo check -p vld-config --no-default-features --features "config-rs"
cargo check -p vld-fake
cargo check -p vld-salvo
cargo check -p vld-redis --no-default-features --features "redis-0"
cargo check -p vld-redis --no-default-features --features "redis-1"
cargo check -p vld-tonic
cargo check -p vld-warp
cargo check -p vld-lapin --no-default-features --features "lapin-2"
cargo check -p vld-lapin --no-default-features --features "lapin-3"
cargo check -p vld-lapin --no-default-features --features "lapin-4"
cargo check -p vld-schemars --no-default-features --features "schemars-0"
cargo check -p vld-schemars --no-default-features --features "schemars-1"
cargo check -p vld-aide --no-default-features --features "schemars-0"
cargo check -p vld-aide --no-default-features --features "schemars-1"

echo "==> Test vld feature matrix"
cargo test -p vld --no-default-features
cargo test -p vld --no-default-features --features serialize
cargo test -p vld --no-default-features --features openapi
cargo test -p vld --no-default-features --features diff
cargo test -p vld --no-default-features --features "serialize,openapi,diff"
cargo test -p vld --features "${VLD_EXTENDED_FEATURES}"

echo "==> Test vld jiff feature matrix"
cargo test -p vld --features "${JIFF_FEATURES}"

echo "==> Test vld time feature matrix"
cargo test -p vld --features "${TIME_FEATURES}"

echo "==> Clippy (workspace, default features, all targets)"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> Clippy (vld extended features)"
cargo clippy -p vld --all-targets --features "${VLD_EXTENDED_FEATURES}" -- -D warnings

echo "==> Clippy (vld jiff features)"
cargo clippy -p vld --all-targets --features "${JIFF_FEATURES}" -- -D warnings

echo "==> Clippy (vld time features)"
cargo clippy -p vld --all-targets --features "${TIME_FEATURES}" -- -D warnings

echo "==> Format check"
cargo fmt --all --check

echo "==> Playground"
cargo run -p playground

echo "==> CI all checks passed"
