#!/usr/bin/env bash
# Full local CI run (also the release preflight) and the command source for
# the GitHub jobs in .github/workflows/ci.yml.
#
#   scripts/ci-all.sh                 # every group
#   scripts/ci-all.sh clippy docs     # only these groups
#
# Groups: test, features, integrations, clippy, docs, deny, msrv, fmt, playground.
# Every step runs even after a failure; a summary is printed at the end and
# the exit code is non-zero if any step failed. Steps whose tool is missing
# locally (cargo-hack, cargo-deny, the MSRV toolchain) are skipped with a note.
#
# - workspace-wide build/test/clippy use `--all-targets`; doctests run
#   separately (`--all-targets` skips them)
# - never `--all-features` on crates with exclusive dual-major features
#   (compile_error!); their branches are listed explicitly below
# - profile `ci` (no debuginfo / no incremental) in a dedicated target dir,
#   removed at the end unless VLD_CI_KEEP_TARGET=1
# - live redis / rabbitmq tests run when REDIS_URL / AMQP_URL are set

set -uo pipefail

VLD_EXTENDED_FEATURES="chrono,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
JIFF_FEATURES="jiff,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
TIME_FEATURES="time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
ALL_DATES_FEATURES="chrono,jiff,time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
MSRV_TOOLCHAIN="${VLD_MSRV_TOOLCHAIN:-1.85}"
MSRV_FEATURES="derive,openapi,regex,diff"

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}" || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT_DIR}/target/ci}"
export CARGO_INCREMENTAL=0
PROFILE=(--profile ci)

FAILED=()
PASSED=0
SKIPPED=()

# step <description> <command...>: run, record the result, never abort.
step() {
  local name="$1"
  shift
  echo
  echo "==> ${name}"
  echo "    $*"
  if "$@"; then
    PASSED=$((PASSED + 1))
  else
    echo "!!! FAILED: ${name}"
    FAILED+=("${name}")
  fi
}

skip() {
  echo
  echo "==> SKIPPED: $1"
  SKIPPED+=("$1")
}

has_tool() {
  command -v "$1" >/dev/null 2>&1
}

cleanup_ci_target() {
  if [[ "${VLD_CI_KEEP_TARGET:-0}" == "1" ]]; then
    echo "==> Keeping CI target dir (${CARGO_TARGET_DIR})"
    return
  fi
  echo "==> Cleaning CI target dir (${CARGO_TARGET_DIR})"
  /bin/rm -rf "${CARGO_TARGET_DIR}"
}

summary() {
  echo
  echo "================ CI summary ================"
  echo "passed:  ${PASSED}"
  echo "skipped: ${#SKIPPED[@]}"
  for s in "${SKIPPED[@]+"${SKIPPED[@]}"}"; do echo "  - ${s}"; done
  echo "failed:  ${#FAILED[@]}"
  for f in "${FAILED[@]+"${FAILED[@]}"}"; do echo "  - ${f}"; done
  echo "============================================"
}

group_test() {
  step "build workspace (all targets)" cargo build --workspace --all-targets "${PROFILE[@]}"
  step "test workspace (all targets)" cargo test --workspace --all-targets "${PROFILE[@]}"
  step "doctests workspace" cargo test --workspace --doc "${PROFILE[@]}"
}

group_features() {
  step "vld: no default features" cargo test -p vld --no-default-features "${PROFILE[@]}"
  step "vld: openapi" cargo test -p vld --no-default-features --features openapi "${PROFILE[@]}"
  step "vld: diff" cargo test -p vld --no-default-features --features diff "${PROFILE[@]}"
  step "vld: regex" cargo test -p vld --no-default-features --features regex "${PROFILE[@]}"
  step "vld: extended (chrono)" cargo test -p vld --features "${VLD_EXTENDED_FEATURES}" "${PROFILE[@]}"
  step "vld: jiff" cargo test -p vld --features "${JIFF_FEATURES}" "${PROFILE[@]}"
  step "vld: time" cargo test -p vld --features "${TIME_FEATURES}" "${PROFILE[@]}"
  step "vld: all date backends" cargo test -p vld --features "${ALL_DATES_FEATURES}" "${PROFILE[@]}"
  if has_tool cargo-hack; then
    step "vld: feature powerset (depth 2)" \
      cargo hack check -p vld --feature-powerset --depth 2 --exclude-features serialize,deserialize "${PROFILE[@]}"
  else
    skip "vld feature powerset (install cargo-hack)"
  fi
}

group_integrations() {
  step "sqlx 0.8 (check)" cargo check -p vld-sqlx --no-default-features --features "sqlx-0_8,sqlite" "${PROFILE[@]}"
  step "sqlx 0.9" cargo test -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite" "${PROFILE[@]}"
  step "sqlx postgres (lib)" cargo test -p vld-sqlx --features postgres --lib "${PROFILE[@]}"
  step "config-rs (check)" cargo check -p vld-config --no-default-features --features "config-rs" "${PROFILE[@]}"
  step "config figment" cargo test -p vld-config --no-default-features --features figment "${PROFILE[@]}"
  step "config config-rs+figment" cargo test -p vld-config --no-default-features --features "config-rs,figment" "${PROFILE[@]}"
  step "redis 0 (check)" cargo check -p vld-redis --no-default-features --features "redis-0" "${PROFILE[@]}"
  step "redis 1" cargo test -p vld-redis --no-default-features --features "redis-1" "${PROFILE[@]}"
  step "redis 0 + aio" cargo test -p vld-redis --no-default-features --features "redis-0,aio" "${PROFILE[@]}"
  step "redis 1 + aio" cargo test -p vld-redis --no-default-features --features "redis-1,aio" "${PROFILE[@]}"
  step "lapin 2" cargo test -p vld-lapin --no-default-features --features "lapin-2" "${PROFILE[@]}"
  step "lapin 3" cargo test -p vld-lapin --no-default-features --features "lapin-3" "${PROFILE[@]}"
  step "lapin 4" cargo test -p vld-lapin --no-default-features --features "lapin-4" "${PROFILE[@]}"
  step "ntex 3" cargo test -p vld-ntex --no-default-features --features ntex-3 "${PROFILE[@]}"
  step "schemars 0 (check)" cargo check -p vld-schemars --no-default-features --features "schemars-0" "${PROFILE[@]}"
  step "schemars 1" cargo test -p vld-schemars --no-default-features --features "schemars-1" "${PROFILE[@]}"
  step "aide schemars 0 (check)" cargo check -p vld-aide --no-default-features --features "schemars-0" "${PROFILE[@]}"
  step "aide schemars 1" cargo test -p vld-aide --no-default-features --features "schemars-1" "${PROFILE[@]}"
  step "sea-orm 1 (check)" cargo check -p vld-sea --no-default-features --features "sea-orm-1" "${PROFILE[@]}"
  # Entity-macro integration tests stay on sea-orm-1; sea-orm-2 has lib unit tests.
  step "sea-orm 2 (lib)" cargo test -p vld-sea --no-default-features --features "sea-orm-2" --lib "${PROFILE[@]}"
  step "sea-orm 1 + all types" cargo test -p vld-sea --features "with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal" "${PROFILE[@]}"
  step "sea-orm 2 + all types (lib)" cargo test -p vld-sea --no-default-features --features "sea-orm-2,with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal" --lib "${PROFILE[@]}"
  step "utoipa 6" cargo test -p vld-utoipa --no-default-features --features "utoipa-6" --tests "${PROFILE[@]}"
}

group_clippy() {
  local deny=(-- -D warnings)
  step "clippy workspace" cargo clippy --workspace --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy vld: no default features" cargo clippy -p vld --all-targets --no-default-features "${PROFILE[@]}" "${deny[@]}"
  step "clippy vld: extended" cargo clippy -p vld --all-targets --features "${VLD_EXTENDED_FEATURES}" "${PROFILE[@]}" "${deny[@]}"
  step "clippy vld: jiff" cargo clippy -p vld --all-targets --features "${JIFF_FEATURES}" "${PROFILE[@]}" "${deny[@]}"
  step "clippy vld: time" cargo clippy -p vld --all-targets --features "${TIME_FEATURES}" "${PROFILE[@]}" "${deny[@]}"
  step "clippy vld: all date backends" cargo clippy -p vld --all-targets --features "${ALL_DATES_FEATURES}" "${PROFILE[@]}" "${deny[@]}"
  step "clippy schemars 1" cargo clippy -p vld-schemars --no-default-features --features schemars-1 --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy aide schemars 1" cargo clippy -p vld-aide --no-default-features --features schemars-1 --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy utoipa 6" cargo clippy -p vld-utoipa --no-default-features --features utoipa-6 --lib --tests "${PROFILE[@]}" "${deny[@]}"
  step "clippy sqlx 0.9" cargo clippy -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite" --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy sea-orm 2" cargo clippy -p vld-sea --no-default-features --features "sea-orm-2,with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal" --lib "${PROFILE[@]}" "${deny[@]}"
  step "clippy redis 1 + aio" cargo clippy -p vld-redis --no-default-features --features "redis-1,aio" --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy lapin 2" cargo clippy -p vld-lapin --no-default-features --features lapin-2 --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy lapin 4" cargo clippy -p vld-lapin --no-default-features --features lapin-4 --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy ntex 3" cargo clippy -p vld-ntex --no-default-features --features ntex-3 --all-targets "${PROFILE[@]}" "${deny[@]}"
  step "clippy config-rs + figment" cargo clippy -p vld-config --no-default-features --features "config-rs,figment" --all-targets "${PROFILE[@]}" "${deny[@]}"
}

group_docs() {
  step "rustdoc (deny warnings)" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps "${PROFILE[@]}"
}

group_deny() {
  if has_tool cargo-deny; then
    step "cargo deny" cargo deny --all-features check
  else
    skip "cargo deny (install cargo-deny)"
  fi
}

# MSRV: core `vld` on the declared rust-version. The lockfile is re-resolved
# by the default (newer) cargo with the MSRV-aware fallback resolver, which
# picks the newest dependency versions compatible with rust-version 1.85;
# the workspace Cargo.lock is restored afterwards.
group_msrv() {
  if ! rustup run "${MSRV_TOOLCHAIN}" rustc --version >/dev/null 2>&1; then
    skip "MSRV ${MSRV_TOOLCHAIN} (rustup toolchain install ${MSRV_TOOLCHAIN})"
    return
  fi
  local backup msrv_target
  backup="$(mktemp)"
  msrv_target="${CARGO_TARGET_DIR}/msrv"
  cp Cargo.lock "${backup}"
  step "MSRV ${MSRV_TOOLCHAIN}: resolve lockfile" \
    env CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile
  step "MSRV ${MSRV_TOOLCHAIN}: test vld" \
    env CARGO_TARGET_DIR="${msrv_target}" cargo "+${MSRV_TOOLCHAIN}" test -p vld --features "${MSRV_FEATURES}" --lib --tests
  cp "${backup}" Cargo.lock
  rm -f "${backup}"
}

group_fmt() {
  step "rustfmt" cargo fmt --all --check
}

group_playground() {
  step "playground" cargo run -p playground "${PROFILE[@]}"
}

ALL_GROUPS=(test features integrations clippy docs deny msrv fmt playground)
SELECTED=("$@")
if [[ ${#SELECTED[@]} -eq 0 ]]; then
  SELECTED=("${ALL_GROUPS[@]}")
fi

trap cleanup_ci_target EXIT
echo "==> CI groups: ${SELECTED[*]} [profile=ci, target=${CARGO_TARGET_DIR}]"
for g in "${SELECTED[@]}"; do
  if ! declare -F "group_${g}" >/dev/null; then
    echo "unknown group: ${g} (known: ${ALL_GROUPS[*]})"
    exit 2
  fi
  "group_${g}"
done

summary
[[ ${#FAILED[@]} -eq 0 ]]
