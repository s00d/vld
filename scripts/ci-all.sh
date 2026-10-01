#!/usr/bin/env bash
set -euo pipefail

# Full local CI run (future-proof for new workspace crates).
#
# Why this stays up to date:
# - workspace-wide build/test/clippy with `--all-targets` (default features)
# - explicit feature-matrix checks for dual-major integrations + core `vld`
# - do NOT use `--all-features` on exclusive dual-feature crates (compile_error!)
#
# Disk: use profile `ci` (no debuginfo / no incremental) and a dedicated
# target dir so a full matrix does not stuff 20–30G into the IDE `target/`.

VLD_EXTENDED_FEATURES="chrono,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"
JIFF_FEATURES="jiff,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"
TIME_FEATURES="time,derive,serialize,openapi,diff,decimal,net,file,string-advanced,file-advanced"

ROOT_DIR="$(git rev-parse --show-toplevel)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target/ci}"
export CARGO_INCREMENTAL=0
PROFILE=(--profile ci)

cleanup_ci_target() {
  if [[ "${VLD_CI_KEEP_TARGET:-0}" == "1" ]]; then
    echo "==> Keeping CI target dir (${CARGO_TARGET_DIR}); size:"
    /usr/bin/du -sh "${CARGO_TARGET_DIR}" 2>/dev/null || true
    return
  fi
  echo "==> Cleaning CI target dir (${CARGO_TARGET_DIR})"
  /bin/rm -rf "${CARGO_TARGET_DIR}"
}

trap cleanup_ci_target EXIT

echo "==> Build workspace (default features, all targets) [profile=ci, target=${CARGO_TARGET_DIR}]"
cargo build --workspace --all-targets "${PROFILE[@]}"

echo "==> Test workspace (default features, all targets)"
cargo test --workspace --all-targets "${PROFILE[@]}"

echo "==> Test integration crates: legacy/new major branches"
cargo check -p vld-sqlx --no-default-features --features "sqlx-0_8,sqlite" "${PROFILE[@]}"
cargo check -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite" "${PROFILE[@]}"
cargo check -p vld-config --no-default-features --features "config-rs" "${PROFILE[@]}"
cargo check -p vld-fake "${PROFILE[@]}"
cargo check -p vld-salvo "${PROFILE[@]}"
cargo check -p vld-redis --no-default-features --features "redis-0" "${PROFILE[@]}"
cargo check -p vld-redis --no-default-features --features "redis-1" "${PROFILE[@]}"
cargo check -p vld-tonic "${PROFILE[@]}"
cargo check -p vld-warp "${PROFILE[@]}"
cargo check -p vld-lapin --no-default-features --features "lapin-2" "${PROFILE[@]}"
cargo check -p vld-lapin --no-default-features --features "lapin-3" "${PROFILE[@]}"
cargo check -p vld-lapin --no-default-features --features "lapin-4" "${PROFILE[@]}"
cargo check -p vld-schemars --no-default-features --features "schemars-0" "${PROFILE[@]}"
cargo check -p vld-schemars --no-default-features --features "schemars-1" "${PROFILE[@]}"
cargo check -p vld-aide --no-default-features --features "schemars-0" "${PROFILE[@]}"
cargo check -p vld-aide --no-default-features --features "schemars-1" "${PROFILE[@]}"
cargo check -p vld-sea --no-default-features --features "sea-orm-1" "${PROFILE[@]}"
# Entity-macro integration tests stay on sea-orm-1; sea-orm-2 has lib unit tests.
cargo test -p vld-sea --no-default-features --features "sea-orm-2" --lib "${PROFILE[@]}"
cargo test -p vld-utoipa --no-default-features --features "utoipa-6" --tests "${PROFILE[@]}"

echo "==> Test vld feature matrix"
cargo test -p vld --no-default-features "${PROFILE[@]}"
cargo test -p vld --no-default-features --features serialize "${PROFILE[@]}"
cargo test -p vld --no-default-features --features openapi "${PROFILE[@]}"
cargo test -p vld --no-default-features --features diff "${PROFILE[@]}"
cargo test -p vld --no-default-features --features "serialize,openapi,diff" "${PROFILE[@]}"
cargo test -p vld --features "${VLD_EXTENDED_FEATURES}" "${PROFILE[@]}"

echo "==> Test vld jiff feature matrix"
cargo test -p vld --features "${JIFF_FEATURES}" "${PROFILE[@]}"

echo "==> Test vld time feature matrix"
cargo test -p vld --features "${TIME_FEATURES}" "${PROFILE[@]}"

echo "==> Clippy (workspace, default features, all targets)"
cargo clippy --workspace --all-targets "${PROFILE[@]}" -- -D warnings

echo "==> Clippy (vld extended features)"
cargo clippy -p vld --all-targets --features "${VLD_EXTENDED_FEATURES}" "${PROFILE[@]}" -- -D warnings

echo "==> Clippy (vld jiff features)"
cargo clippy -p vld --all-targets --features "${JIFF_FEATURES}" "${PROFILE[@]}" -- -D warnings

echo "==> Clippy (vld time features)"
cargo clippy -p vld --all-targets --features "${TIME_FEATURES}" "${PROFILE[@]}" -- -D warnings

echo "==> Format check"
cargo fmt --all --check

echo "==> Playground"
cargo run -p playground "${PROFILE[@]}"

echo "==> CI all checks passed"
