# Development and CI tasks. `just` lists them; `just ci` is what CI runs on every push.
# Tools: `cargo install just cargo-hack cargo-deny cargo-shear cargo-semver-checks`.
#
# Builds use the `ci` profile (no debuginfo), so everything lands in target/ci and
# `just clean` frees it without touching the dev build. Every build is `--locked`.

set shell := ["bash", "-euo", "pipefail", "-c"]

ci_flags := "--profile ci --locked"

# Crates with optional features, run over feature pairs: `crate:majors`. The majors
# (one feature per supported major of the dependency, `compile_error!` unless exactly
# one is on) never combine. vld-diesel postgres/mysql need libpq / libmysqlclient.
feature_crates := "vld-config: vld-diesel: vld-http-common: vld-utoipa:utoipa-5,utoipa-6 vld-schemars:schemars-0,schemars-1 vld-aide:schemars-0,schemars-1 vld-redis:redis-0,redis-1 vld-lapin:lapin-2,lapin-3,lapin-4 vld-ntex:ntex-3,ntex-4 vld-sqlx:sqlx-0_8,sqlx-0_9 vld-sea:sea-orm-1,sea-orm-2"
# vld-sea's column-type features are tested together, not one by one.
sea_types := "with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal,with-time,postgres-array"
vld_all := "chrono,jiff,time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced"

[private]
default:
    @just --list

# Everything the push/PR CI job runs
ci: fmt lint test docs deny package fuzz playground

# rustfmt check
fmt:
    cargo fmt --all --check

# Unused or misplaced dependencies, then clippy -D warnings: workspace, every vld feature
# alone and together, feature pairs of integration crates
lint:
    cargo shear
    cargo clippy --workspace --all-targets {{ ci_flags }} -- -D warnings
    cargo hack clippy -p vld --each-feature --exclude-features serialize,deserialize --all-targets {{ ci_flags }} -- -D warnings
    cargo clippy -p vld --features {{ vld_all }} --all-targets {{ ci_flags }} -- -D warnings
    just _majors clippy --all-targets -- -D warnings

# Tests: workspace + doctests, vld per feature and with all features, feature pairs of integration crates
test:
    cargo test --workspace --all-targets {{ ci_flags }}
    cargo test --workspace --doc {{ ci_flags }}
    cargo hack test -p vld --each-feature --exclude-features serialize,deserialize {{ ci_flags }}
    cargo test -p vld --features {{ vld_all }} {{ ci_flags }}
    just _majors test

# `cargo hack <cmd>` over feature pairs of each crate in feature_crates
[private]
_majors cmd *args:
    #!/usr/bin/env bash
    set -euo pipefail
    for entry in {{ feature_crates }}; do
      crate="${entry%%:*}" majors="${entry#*:}"
      flags=()
      [[ -n "${majors}" ]] && flags+=(--exclude-features default --mutually-exclusive-features "${majors}" --at-least-one-of "${majors}")
      [[ "${crate}" == vld-sea ]] && flags+=(--group-features "{{ sea_types }}")
      cargo hack {{ cmd }} -p "${crate}" --feature-powerset --depth 2 "${flags[@]}" {{ ci_flags }} {{ args }}
    done

# rustdoc with warnings denied
docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps {{ ci_flags }}

# Advisories, licenses, duplicate and banned crates (deny.toml)
deny:
    cargo deny --workspace --all-features check

# What `cargo publish` would upload (--no-verify: dependents of unpublished crates can't build from the registry)
package *args:
    cargo package --workspace --exclude playground --no-verify --locked {{ args }}

# Fuzz targets compile (fuzz/ is its own workspace)
fuzz:
    cargo check --manifest-path fuzz/Cargo.toml --locked

# Run the playground example
playground:
    cargo run -p playground {{ ci_flags }}

# vld, vld-leptos and vld-dioxus build for wasm32 (rustup target add wasm32-unknown-unknown)
wasm:
    cargo check --target wasm32-unknown-unknown -p vld -p vld-leptos -p vld-dioxus {{ ci_flags }}

# Every pair of vld features compiles (slow: ~100 builds)
powerset:
    cargo hack check -p vld --feature-powerset --depth 2 --exclude-features serialize,deserialize --all-targets {{ ci_flags }}

# Each crate on its rust-version, as a downstream user resolves it (MSRV.md; needs those toolchains)
msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${CARGO_TARGET_DIR:-$PWD/target}/msrv"
    # probe <toolchain> <crate> [features] [no-default]: a throwaway project that depends
    # only on <crate>; edition 2024 (resolver 3) resolves its lockfile for its
    # rust-version (resolving the whole workspace would pull versions other members
    # need, e.g. rama pins time =0.3.47 / Rust 1.88).
    probe() {
      local tc="$1" crate="$2" features="${3:-}" nodefault="${4:-}" dir dep path="$PWD/crates/$2"
      echo "==> MSRV ${tc}: ${crate}${features:+ [${features}]}"
      dir="$(mktemp -d)"; mkdir "${dir}/src"; echo 'fn main() {}' > "${dir}/src/main.rs"
      dep="path = \"${path}\""
      [[ -n "${features}" ]] && dep+=", features = [\"${features//,/\", \"}\"]"
      [[ -n "${nodefault}" ]] && dep+=", default-features = false"
      printf '[package]\nname = "msrv-probe"\nversion = "0.0.0"\nedition = "2024"\nrust-version = "%s"\npublish = false\n[dependencies]\n%s = { %s }\n[workspace]\n' \
        "${tc}" "${crate}" "${dep}" > "${dir}/Cargo.toml"
      # --ignore-rust-version: vld-ntex declares 1.97 (ntex 4) but ntex-3 is probed on 1.88.
      (cd "${dir}" && cargo generate-lockfile -q \
        && CARGO_TARGET_DIR="${target}-${tc}" cargo "+${tc}" check -q --locked --ignore-rust-version)
      rm -rf "${dir}"
    }
    probe 1.85 vld "{{ vld_all }}"
    cargo metadata --no-deps --format-version 1 --locked | python3 -c '
    import json, sys
    for p in json.load(sys.stdin)["packages"]:
        if p["publish"] != [] and p["name"] != "vld":
            print(p["name"], p["rust_version"])' | while read -r crate tc; do probe "${tc}" "${crate}"; done
    probe 1.88 vld-utoipa utoipa-6 no-default
    probe 1.88 vld-lapin lapin-4 no-default
    probe 1.88 vld-ntex ntex-3 no-default
    probe 1.94 vld-sea "sea-orm-2,{{ sea_types }}" no-default
    probe 1.94 vld-sqlx "sqlx-0_9,postgres,mysql,sqlite" no-default

# Lowest allowed versions of direct dependencies compile (nightly; crates with wide version ranges)
minimal:
    #!/usr/bin/env bash
    set -euo pipefail
    # A scratch workspace with just these crates and no dev-dependencies: the whole
    # workspace can't resolve at minimal versions (rama pins time =0.3.47).
    members=(crates/vld crates/vld-derive crates/vld-http-common crates/vld-warp crates/vld-tonic crates/vld-config crates/vld-redis crates/vld-schemars crates/vld-aide)
    scratch="$(mktemp -d)"; trap 'rm -rf "${scratch}"' EXIT
    git ls-files -z | grep -zv '^fuzz/\|^docs/' | xargs -0 tar -cf - | tar -xf - -C "${scratch}"
    list="$(printf '"%s", ' "${members[@]}")"
    python3 - "${scratch}" "${list%, }" "${members[@]}" <<'PY'
    import re, sys
    root, members, crates = sys.argv[1], sys.argv[2], sys.argv[3:]
    ws = f"{root}/Cargo.toml"
    text = open(ws).read()
    open(ws, "w").write(re.sub(r"^members = \[.*?^\]", f"members = [{members}]", text, count=1, flags=re.S | re.M))
    for c in crates:
        path = f"{root}/{c}/Cargo.toml"
        text = open(path).read()
        open(path, "w").write(re.sub(r"^\[dev-dependencies\].*?(?=^\[|\Z)", "", text, flags=re.S | re.M))
    PY
    m="${scratch}/Cargo.toml"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}/minimal"
    cargo +nightly update -Z direct-minimal-versions --manifest-path "$m"
    cargo +nightly check --locked --manifest-path "$m" -p vld-warp -p vld-tonic -p vld-schemars -p vld-aide
    cargo +nightly check --locked --manifest-path "$m" -p vld-config --features figment
    cargo +nightly check --locked --manifest-path "$m" -p vld-redis --features aio-tokio

# Public API changes vs the latest release on crates.io match the version bump
# (default features: the semver-checks heuristic would enable two dependency majors at once)
semver:
    cargo semver-checks -p vld --only-explicit-features --features {{ vld_all }}
    cargo semver-checks --workspace --exclude vld --exclude playground --default-features

# Remove CI build output (target/ci, target/msrv-*, target/minimal)
clean:
    rm -rf target/ci target/msrv-* target/minimal

# cargo-release pre-release hook (runs after the version bump): checks (semver-checks
# compares the API with crates.io for the new version), then
# [Unreleased] -> [version] in CHANGELOG.md, and fuzz/Cargo.lock (fuzz/ is outside
# the workspace) pointed at the new version.
[private]
release-hook version:
    just fmt lint test docs semver
    just package --allow-dirty
    @if [[ "${DRY_RUN:-false}" == "true" ]]; then echo "dry run: CHANGELOG.md unchanged"; else just _changelog {{ version }}; fi
    cargo update --manifest-path fuzz/Cargo.toml --workspace --offline

[private]
_changelog version:
    #!/usr/bin/env python3
    import datetime, re, sys
    from pathlib import Path
    path, version = Path("CHANGELOG.md"), "{{ version }}"
    text = path.read_text()
    if re.search(rf"^## \[{re.escape(version)}\]", text, re.M):
        sys.exit(0)
    m = re.search(r"^## \[Unreleased\][^\n]*\n", text, re.M)
    rest = text[m.end():] if m else ""
    nxt = re.search(r"^## \[", rest, re.M)
    if not m or not rest[: nxt.start() if nxt else None].strip():
        sys.exit("CHANGELOG.md: write the release notes under '## [Unreleased]' first")
    path.write_text(text[: m.start()] + f"## [{version}] - {datetime.date.today()}\n" + rest)
    print(f"CHANGELOG.md: [Unreleased] -> [{version}]")
