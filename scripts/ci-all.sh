#!/usr/bin/env bash
# Full local CI run and the command source for the GitHub jobs in
# .github/workflows/ci.yml. The release preflight runs a subset (see
# scripts/release-preflight.sh).
#
#   scripts/ci-all.sh                 # every group
#   scripts/ci-all.sh clippy docs     # only these groups
#
# Groups: test, features, integrations, clippy, docs, deny, msrv, minimal,
# fmt, package, fuzz, wasm, playground.
# Every step runs even after a failure; a summary is printed at the end and
# the exit code is non-zero if any step failed. Steps whose tool, toolchain,
# target or system library is missing locally are skipped with a note.
#
# - every build uses the committed Cargo.lock (`--locked`); the msrv and
#   minimal groups resolve their own lockfiles in scratch copies (the
#   workspace Cargo.lock is never rewritten)
# - workspace-wide build/test/clippy use `--all-targets`; doctests run
#   separately (`--all-targets` skips them)
# - never `--all-features` on crates with exclusive dual-major features
#   (compile_error!); their branches are listed explicitly below
# - profile `ci` (no debuginfo / no incremental); the default target dir
#   (target/ci) is removed at the end unless VLD_CI_KEEP_TARGET=1. A caller's
#   CARGO_TARGET_DIR is never removed.
# - live redis / rabbitmq tests run when REDIS_URL / AMQP_URL are set

set -uo pipefail

VLD_EXTENDED_FEATURES="chrono,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
JIFF_FEATURES="jiff,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
TIME_FEATURES="time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
ALL_DATES_FEATURES="chrono,jiff,time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"
MSRV_TOOLCHAIN="${VLD_MSRV_TOOLCHAIN:-1.85}"
# Every vld feature builds on the core MSRV (see MSRV.md).
MSRV_FEATURES="${ALL_DATES_FEATURES}"
SEA_TYPES="with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal,with-time,postgres-array"

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}" || exit 1
OWN_TARGET=0
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  export CARGO_TARGET_DIR="${ROOT_DIR}/target/ci"
  OWN_TARGET=1
elif [[ "${CARGO_TARGET_DIR}" != /* ]]; then
  # The msrv group builds from scratch directories.
  export CARGO_TARGET_DIR="${ROOT_DIR}/${CARGO_TARGET_DIR}"
fi
export CARGO_INCREMENTAL=0
CI=(--profile ci --locked)

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

has_toolchain() {
  rustup run "$1" rustc --version >/dev/null 2>&1
}

on_exit() {
  if [[ "${OWN_TARGET}" != "1" ]]; then
    return
  fi
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
  step "build workspace (all targets)" cargo build --workspace --all-targets "${CI[@]}"
  step "test workspace (all targets)" cargo test --workspace --all-targets "${CI[@]}"
  step "doctests workspace" cargo test --workspace --doc "${CI[@]}"
}

group_features() {
  # Default features are empty, so the plain vld run is in `test`.
  step "vld: openapi" cargo test -p vld --no-default-features --features openapi "${CI[@]}"
  step "vld: diff" cargo test -p vld --no-default-features --features diff "${CI[@]}"
  step "vld: regex" cargo test -p vld --no-default-features --features regex "${CI[@]}"
  step "vld: extended (chrono)" cargo test -p vld --features "${VLD_EXTENDED_FEATURES}" "${CI[@]}"
  step "vld: jiff" cargo test -p vld --features "${JIFF_FEATURES}" "${CI[@]}"
  step "vld: time" cargo test -p vld --features "${TIME_FEATURES}" "${CI[@]}"
  step "vld: all date backends" cargo test -p vld --features "${ALL_DATES_FEATURES}" "${CI[@]}"
  if has_tool cargo-hack; then
    step "vld: feature powerset (depth 2, all targets)" \
      cargo hack check -p vld --feature-powerset --depth 2 --all-targets --exclude-features serialize,deserialize "${CI[@]}"
  else
    skip "vld feature powerset (install cargo-hack)"
  fi
}

# Database client libraries for the diesel postgres/mysql backends
# (CI: apt-get install libpq-dev libmysqlclient-dev).
has_libpq() { has_tool pg_config || pkg-config --exists libpq 2>/dev/null; }
has_libmysql() { has_tool mysql_config || pkg-config --exists mysqlclient 2>/dev/null; }

group_integrations() {
  step "sqlx 0.8 (check)" cargo check -p vld-sqlx --no-default-features --features "sqlx-0_8,sqlite" "${CI[@]}"
  step "sqlx 0.9" cargo test -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite" "${CI[@]}"
  step "sqlx postgres (lib)" cargo test -p vld-sqlx --features postgres --lib "${CI[@]}"
  step "sqlx 0.8 mysql (check)" cargo check -p vld-sqlx --features mysql "${CI[@]}"
  step "sqlx 0.9 all backends (check)" cargo check -p vld-sqlx --no-default-features --features "sqlx-0_9,postgres,mysql,sqlite" "${CI[@]}"
  if has_libpq; then
    step "diesel postgres (check)" cargo check -p vld-diesel --features postgres --all-targets "${CI[@]}"
  else
    skip "diesel postgres (needs libpq: apt libpq-dev / brew libpq)"
  fi
  if has_libmysql; then
    step "diesel mysql (check)" cargo check -p vld-diesel --features mysql --all-targets "${CI[@]}"
  else
    skip "diesel mysql (needs libmysqlclient: apt libmysqlclient-dev / brew mysql-client)"
  fi
  step "config-rs (check)" cargo check -p vld-config --no-default-features --features "config-rs" "${CI[@]}"
  step "config figment" cargo test -p vld-config --no-default-features --features figment "${CI[@]}"
  step "config config-rs+figment" cargo test -p vld-config --no-default-features --features "config-rs,figment" "${CI[@]}"
  step "redis 0 (check)" cargo check -p vld-redis --no-default-features --features "redis-0" "${CI[@]}"
  step "redis 1" cargo test -p vld-redis --no-default-features --features "redis-1" "${CI[@]}"
  step "redis 0 + aio" cargo test -p vld-redis --no-default-features --features "redis-0,aio" "${CI[@]}"
  step "redis 1 + aio" cargo test -p vld-redis --no-default-features --features "redis-1,aio" "${CI[@]}"
  step "redis 1 + aio-tokio" cargo test -p vld-redis --no-default-features --features "redis-1,aio-tokio" "${CI[@]}"
  step "lapin 2" cargo test -p vld-lapin --no-default-features --features "lapin-2" "${CI[@]}"
  step "lapin 3" cargo test -p vld-lapin --no-default-features --features "lapin-3" "${CI[@]}"
  step "lapin 4" cargo test -p vld-lapin --no-default-features --features "lapin-4" "${CI[@]}"
  step "ntex 3" cargo test -p vld-ntex --no-default-features --features ntex-3 "${CI[@]}"
  if [[ -n "${VLD_TS_NODE_DIR:-}" ]]; then
    step "vld-ts node cross-check" cargo test -p vld-ts --test regression "${CI[@]}"
  else
    skip "vld-ts node cross-check (set VLD_TS_NODE_DIR to a dir with zod@3 + valibot@1)"
  fi
  step "ntex 4 + compress" cargo test -p vld-ntex --features compress "${CI[@]}"
  step "ntex 3 + compress" cargo test -p vld-ntex --no-default-features --features ntex-3,compress "${CI[@]}"
  step "schemars 0 (check)" cargo check -p vld-schemars --no-default-features --features "schemars-0" "${CI[@]}"
  step "schemars 1" cargo test -p vld-schemars --no-default-features --features "schemars-1" "${CI[@]}"
  step "aide schemars 0 (check)" cargo check -p vld-aide --no-default-features --features "schemars-0" "${CI[@]}"
  step "aide schemars 1" cargo test -p vld-aide --no-default-features --features "schemars-1" "${CI[@]}"
  step "sea-orm 1 (check)" cargo check -p vld-sea --no-default-features --features "sea-orm-1" "${CI[@]}"
  step "sea-orm 2" cargo test -p vld-sea --no-default-features --features "sea-orm-2" "${CI[@]}"
  step "sea-orm 1 + all types" cargo test -p vld-sea --features "${SEA_TYPES}" "${CI[@]}"
  step "sea-orm 2 + all types (lib)" cargo test -p vld-sea --no-default-features --features "sea-orm-2,${SEA_TYPES}" --lib "${CI[@]}"
  step "utoipa 6" cargo test -p vld-utoipa --no-default-features --features "utoipa-6" --tests "${CI[@]}"
}

group_clippy() {
  local deny=(-- -D warnings)
  step "clippy workspace" cargo clippy --workspace --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy vld: extended" cargo clippy -p vld --all-targets --features "${VLD_EXTENDED_FEATURES}" "${CI[@]}" "${deny[@]}"
  step "clippy vld: jiff" cargo clippy -p vld --all-targets --features "${JIFF_FEATURES}" "${CI[@]}" "${deny[@]}"
  step "clippy vld: time" cargo clippy -p vld --all-targets --features "${TIME_FEATURES}" "${CI[@]}" "${deny[@]}"
  step "clippy vld: all date backends" cargo clippy -p vld --all-targets --features "${ALL_DATES_FEATURES}" "${CI[@]}" "${deny[@]}"
  step "clippy schemars 1" cargo clippy -p vld-schemars --no-default-features --features schemars-1 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy aide schemars 1" cargo clippy -p vld-aide --no-default-features --features schemars-1 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy utoipa 6" cargo clippy -p vld-utoipa --no-default-features --features utoipa-6 --lib --tests "${CI[@]}" "${deny[@]}"
  step "clippy sqlx 0.9" cargo clippy -p vld-sqlx --no-default-features --features "sqlx-0_9,sqlite" --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy sea-orm 1 + all types" cargo clippy -p vld-sea --features "${SEA_TYPES}" --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy sea-orm 2" cargo clippy -p vld-sea --no-default-features --features "sea-orm-2,${SEA_TYPES}" --lib "${CI[@]}" "${deny[@]}"
  step "clippy redis 0 + aio" cargo clippy -p vld-redis --no-default-features --features "redis-0,aio" --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy redis 1 + aio" cargo clippy -p vld-redis --no-default-features --features "redis-1,aio" --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy lapin 2" cargo clippy -p vld-lapin --no-default-features --features lapin-2 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy lapin 3" cargo clippy -p vld-lapin --no-default-features --features lapin-3 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy lapin 4" cargo clippy -p vld-lapin --no-default-features --features lapin-4 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy ntex 3" cargo clippy -p vld-ntex --no-default-features --features ntex-3 --all-targets "${CI[@]}" "${deny[@]}"
  step "clippy config-rs + figment" cargo clippy -p vld-config --no-default-features --features "config-rs,figment" --all-targets "${CI[@]}" "${deny[@]}"
}

group_docs() {
  step "rustdoc (deny warnings)" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps "${CI[@]}"
}

group_deny() {
  if has_tool cargo-deny; then
    # --workspace: without it only the root `vld` package is checked.
    step "cargo deny" cargo deny --workspace --all-features check
  else
    skip "cargo deny (install cargo-deny)"
  fi
}

# msrv_probe <toolchain> <crate> [features] [no-default]: check <crate> the
# way a downstream user on <toolchain> sees it: a throwaway project that
# depends only on that crate, its lockfile resolved by the MSRV-aware
# fallback resolver (workspace-wide unification would drag in versions
# needed by other members, e.g. rama pins time =0.3.47 / Rust 1.88).
msrv_probe() {
  local tc="$1" crate="$2" features="${3:-}" nodefault="${4:-}"
  local label="MSRV ${tc}: ${crate}${features:+ [${features}]}${nodefault:+ (no default)}"
  if ! has_toolchain "${tc}"; then
    skip "${label} (rustup toolchain install ${tc} --profile minimal)"
    return
  fi
  local path="${ROOT_DIR}/crates/${crate}" dir dep
  [[ "${crate}" == "vld" ]] && path="${ROOT_DIR}"
  dir="$(mktemp -d)"
  dep="path = \"${path}\""
  if [[ -n "${features}" ]]; then
    dep+=", features = [\"${features//,/\", \"}\"]"
  fi
  [[ -n "${nodefault}" ]] && dep+=", default-features = false"
  mkdir -p "${dir}/src"
  echo "fn main() {}" >"${dir}/src/main.rs"
  printf '[package]\nname = "msrv-probe"\nversion = "0.0.0"\nedition = "2021"\nrust-version = "%s"\npublish = false\n\n[dependencies]\n%s = { %s }\n\n[workspace]\n' \
    "${tc}" "${crate}" "${dep}" >"${dir}/Cargo.toml"
  # --ignore-rust-version: vld-ntex declares 1.97 (ntex 4) but is probed
  # with ntex-3 on 1.88; for every other probe <toolchain> is the declared one.
  step "${label}" bash -c "cd '${dir}' \
    && CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile -q \
    && CARGO_TARGET_DIR='${CARGO_TARGET_DIR}/msrv-${tc}' cargo +${tc} check -q --locked --ignore-rust-version"
  rm -rf "${dir}"
}

# MSRV (table in MSRV.md):
# - core vld: lib + tests with every feature on ${MSRV_TOOLCHAIN}, in a
#   scratch copy of the root package whose lockfile is re-resolved with the
#   fallback resolver (the workspace Cargo.lock is never touched);
# - every published crate: its default features on its own rust-version,
#   plus the non-default majors that need a newer Rust.
group_msrv() {
  if has_toolchain "${MSRV_TOOLCHAIN}"; then
    local scratch
    scratch="$(mktemp -d)"
    cp -R Cargo.toml Cargo.lock README.md LICENSE src tests benches "${scratch}/"
    mkdir -p "${scratch}/crates"
    cp -R crates/vld-derive "${scratch}/crates/"
    # Only vld + vld-derive as members: other crates must not raise versions.
    awk 'BEGIN{skip=0} /^members = \[/{print "members = [\".\", \"crates/vld-derive\"]"; skip=1; next}
         skip && /^\]/{skip=0; next} !skip' Cargo.toml >"${scratch}/Cargo.toml"
    step "MSRV ${MSRV_TOOLCHAIN}: vld lib+tests [all features]" bash -c "cd '${scratch}' \
      && CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile -q \
      && CARGO_TARGET_DIR='${CARGO_TARGET_DIR}/msrv-${MSRV_TOOLCHAIN}' \
         cargo +${MSRV_TOOLCHAIN} test --locked -p vld --features '${MSRV_FEATURES}' --lib --tests"
    rm -rf "${scratch}"
  else
    skip "MSRV ${MSRV_TOOLCHAIN}: vld (rustup toolchain install ${MSRV_TOOLCHAIN} --profile minimal)"
  fi

  local crate tc
  while read -r crate tc; do
    msrv_probe "${tc}" "${crate}"
  done < <(cargo metadata --no-deps --format-version 1 --locked | python3 -c '
import json, sys
for p in json.load(sys.stdin)["packages"]:
    if p["publish"] != [] and p["name"] != "vld":
        print(p["name"], p["rust_version"])')
  msrv_probe 1.88 vld-utoipa utoipa-6 no-default
  msrv_probe 1.88 vld-lapin lapin-4 no-default
  msrv_probe 1.88 vld-ntex ntex-3 no-default
  msrv_probe 1.94 vld-sea "sea-orm-2,${SEA_TYPES}" no-default
  msrv_probe 1.94 vld-sqlx "sqlx-0_9,postgres,mysql,sqlite" no-default
}

# Lowest versions of the direct (normal) dependencies of the crates with
# wide version ranges (warp, tonic, config, redis 0.x, schemars 0.x) and of
# the core crates they pull in. Nightly only. Runs in a scratch copy whose
# workspace holds just those crates without dev-dependencies: the whole
# workspace cannot resolve at direct-minimal versions (rama pins
# time =0.3.47, dev-dependency floors differ between crates).
MINIMAL_MEMBERS=(. crates/vld-derive crates/vld-http-common crates/vld-warp crates/vld-tonic
  crates/vld-config crates/vld-redis crates/vld-schemars crates/vld-aide)
group_minimal() {
  if ! has_toolchain nightly; then
    skip "minimal versions (rustup toolchain install nightly --profile minimal)"
    return
  fi
  local scratch members m
  scratch="$(mktemp -d)"
  tar -cf - --exclude=./target --exclude=./.git --exclude=./fuzz . | tar -xf - -C "${scratch}"
  members=""
  for m in "${MINIMAL_MEMBERS[@]}"; do members+="\"${m}\", "; done
  (
    cd "${scratch}" || exit 1
    awk -v members="members = [${members%, }]" 'BEGIN{skip=0}
      /^members = \[/{print members; skip=1; next} skip && /^\]/{skip=0; next} !skip' \
      Cargo.toml >Cargo.toml.new && mv Cargo.toml.new Cargo.toml
    for m in "${MINIMAL_MEMBERS[@]}"; do
      awk '/^\[dev-dependencies\]/{skip=1; next} /^\[/{skip=0} !skip' "${m}/Cargo.toml" >"${m}/Cargo.toml.new" \
        && mv "${m}/Cargo.toml.new" "${m}/Cargo.toml"
    done
  )
  local min=(env CARGO_TARGET_DIR="${CARGO_TARGET_DIR}/minimal" cargo +nightly check --locked --manifest-path "${scratch}/Cargo.toml")
  step "minimal versions: resolve" cargo +nightly update -Z direct-minimal-versions --manifest-path "${scratch}/Cargo.toml"
  if [[ "${FAILED[*]-}" != *"minimal versions: resolve"* ]]; then
    step "minimal: warp" "${min[@]}" -p vld-warp
    step "minimal: tonic" "${min[@]}" -p vld-tonic
    step "minimal: config-rs + figment" "${min[@]}" -p vld-config --features figment
    step "minimal: redis 0 + aio" "${min[@]}" -p vld-redis --features aio-tokio
    step "minimal: schemars 0 / aide" "${min[@]}" -p vld-schemars -p vld-aide
  fi
  rm -rf "${scratch}"
}

group_fmt() {
  step "rustfmt" cargo fmt --all --check
}

# What `cargo publish` would upload (file lists are checked by eye with
# `cargo package -p <crate> --list`). --no-verify: dependents of a crate that
# is not on crates.io yet cannot be built from the registry.
# VLD_CI_ALLOW_DIRTY=1: the release preflight runs after cargo-release has
# bumped the versions, before it commits.
group_package() {
  local dirty=()
  [[ "${VLD_CI_ALLOW_DIRTY:-0}" == "1" ]] && dirty=(--allow-dirty)
  step "package workspace" cargo package --workspace --exclude playground --no-verify --locked "${dirty[@]+"${dirty[@]}"}"
}

group_fuzz() {
  step "fuzz targets (check)" cargo check --manifest-path fuzz/Cargo.toml --locked
}

group_wasm() {
  if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
    step "wasm32: vld, vld-leptos, vld-dioxus (check)" \
      cargo check --target wasm32-unknown-unknown -p vld -p vld-leptos -p vld-dioxus "${CI[@]}"
  else
    skip "wasm32 check (rustup target add wasm32-unknown-unknown)"
  fi
}

group_playground() {
  step "playground" cargo run -p playground "${CI[@]}"
}

ALL_GROUPS=(test features integrations clippy docs deny msrv minimal fmt package fuzz wasm playground)
SELECTED=("$@")
if [[ ${#SELECTED[@]} -eq 0 ]]; then
  SELECTED=("${ALL_GROUPS[@]}")
fi

for g in "${SELECTED[@]}"; do
  if ! declare -F "group_${g}" >/dev/null; then
    echo "unknown group: ${g} (known: ${ALL_GROUPS[*]})"
    exit 2
  fi
done

trap on_exit EXIT
trap 'exit 130' INT TERM
echo "==> CI groups: ${SELECTED[*]} [profile=ci, target=${CARGO_TARGET_DIR}]"
for g in "${SELECTED[@]}"; do
  "group_${g}"
done

summary
[[ ${#FAILED[@]} -eq 0 ]]
