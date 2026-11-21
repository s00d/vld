# Development and CI tasks. `just` lists them; `just ci` is what CI runs on every push.
# Tools: `cargo install just cargo-hack cargo-deny cargo-shear cargo-semver-checks cargo-sweep`.
#
# Builds use the `ci` profile (no debuginfo), so everything lands in target/ci and
# `just clean` frees it without touching the dev build. Every build is `--locked`.

set shell := ["bash", "-euo", "pipefail", "-c"]

ci_flags := "--profile ci --locked"

# Crate groups (ARCHITECTURE.md): `core` = vld + vld-derive, the others = vld-<group>.
# CI runs one job per group. vld-data's diesel postgres/mysql need libpq / libmysqlclient.
groups := "core web openapi data app"
vld_all := "chrono,jiff,time,derive,regex,openapi,diff,decimal,net,file,string-advanced,file-advanced,ts,fake"
# vld-data without the postgres / mysql backends (diesel links libpq / libmysqlclient,
# which only the Linux CI image has): what `test-group data` runs on macOS / Windows.
data_portable := "sqlx-0_8,sqlx-0_9,diesel,sqlite,sea-orm-1,sea-orm-2,with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal,with-time,postgres-array,redis-0,redis-1,redis-aio-tokio,lapin-2,lapin-3,lapin-4"

[private]
default:
    @just --list

# Everything CI runs on Linux (there split into lint / test jobs per group plus `misc`)
ci: misc lint test trim

# The checks outside the crate groups
misc: fmt docs deny package fuzz playground
    cargo shear

# rustfmt check
fmt:
    cargo fmt --all --check

# clippy -D warnings for every group
lint:
    for g in {{ groups }}; do just lint-group "${g}"; done

# Tests for every group
test:
    for g in {{ groups }}; do just test-group "${g}"; done

# Lint and test one group: what one CI job runs
group name: (lint-group name) (test-group name)

# clippy for one group: all features, then each feature alone (core with its tests)
lint-group name:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ name }}" in
      core)
        cargo clippy -p vld -p vld-derive -p playground --all-features --all-targets {{ ci_flags }} -- -D warnings
        cargo hack clippy -p vld --each-feature --all-targets {{ ci_flags }} -- -D warnings ;;
      web|openapi|data|app)
        cargo clippy -p vld-{{ name }} --all-features --all-targets {{ ci_flags }} -- -D warnings
        cargo hack clippy -p vld-{{ name }} --each-feature --lib {{ ci_flags }} -- -D warnings ;;
      *) echo "unknown group '{{ name }}' (groups: {{ groups }})"; exit 1 ;;
    esac

# Unit, integration and doc tests with all features (core: also without default
# features; data off Linux: without postgres / mysql)
test-group name:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ name }}" in
      core)
        cargo test -p vld -p vld-derive --all-features {{ ci_flags }}
        cargo test -p vld --no-default-features {{ ci_flags }} ;;
      data)
        if [[ "$(uname -s)" == Linux ]]; then features=(--all-features); else features=(--features "{{ data_portable }}"); fi
        cargo test -p vld-data "${features[@]}" {{ ci_flags }} ;;
      web|openapi|app)
        cargo test -p vld-{{ name }} --all-features {{ ci_flags }} ;;
      *) echo "unknown group '{{ name }}' (groups: {{ groups }})"; exit 1 ;;
    esac

# rustdoc with warnings denied, every feature (what docs.rs builds)
docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps {{ ci_flags }}

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

# vld (the `form` module included) builds for wasm32 (rustup target add wasm32-unknown-unknown)
wasm:
    cargo check --target wasm32-unknown-unknown -p vld {{ ci_flags }}

# Every pair of vld features compiles (slow: ~100 builds)
powerset:
    cargo hack check -p vld --feature-powerset --depth 2 --all-targets {{ ci_flags }}

# Each crate on its rust-version, as a downstream user resolves it (MSRV.md; needs those toolchains)
msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    target="${CARGO_TARGET_DIR:-$PWD/target}/msrv"
    # probe <toolchain> <crate> [features]: a throwaway project that depends
    # only on <crate>; edition 2024 (resolver 3) resolves its lockfile for its
    # rust-version (resolving the whole workspace would pull versions other members
    # need, e.g. rama pins time =0.3.47 / Rust 1.88).
    probe() {
      local tc="$1" crate="$2" features="${3:-}" dir dep path="$PWD/crates/$2"
      echo "==> MSRV ${tc}: ${crate}${features:+ [${features}]}"
      dir="$(mktemp -d)"; mkdir "${dir}/src"; echo 'fn main() {}' > "${dir}/src/main.rs"
      dep="path = \"${path}\""
      [[ -n "${features}" ]] && dep+=", features = [\"${features//,/\", \"}\"]"
      printf '[package]\nname = "msrv-probe"\nversion = "0.0.0"\nedition = "2024"\nrust-version = "%s"\npublish = false\n[dependencies]\n%s = { %s }\n[workspace]\n' \
        "${tc}" "${crate}" "${dep}" > "${dir}/Cargo.toml"
      (cd "${dir}" && cargo generate-lockfile -q \
        && CARGO_TARGET_DIR="${target}-${tc}" cargo "+${tc}" check -q --locked)
      rm -rf "${dir}"
    }
    probe 1.85 vld "{{ vld_all }}"
    probe 1.85 vld-derive
    probe 1.85 vld-web axum,actix,poem,rocket,warp,tower,tonic
    probe 1.88 vld-web ntex-3
    probe 1.94 vld-web salvo
    probe 1.96 vld-web rama
    probe 1.97 vld-web ntex-4,ntex-compress
    probe 1.85 vld-openapi utoipa-5,schemars-0,schemars-1
    probe 1.88 vld-openapi utoipa-6
    probe 1.85 vld-data "sqlx-0_8,diesel,sqlite,sea-orm-1,with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal,with-time,postgres-array,redis-0,redis-1,redis-aio-tokio,lapin-2,lapin-3"
    probe 1.94 vld-data "sqlx-0_9,postgres,mysql,sqlite"
    probe 1.94 vld-data "sea-orm-2,with-json,with-chrono,with-uuid,with-rust_decimal,with-bigdecimal,with-time,postgres-array"
    probe 1.88 vld-data lapin-4
    probe 1.85 vld-app config-rs,figment,clap

# Lowest allowed versions of direct dependencies compile (nightly; crates with wide version ranges)
minimal:
    #!/usr/bin/env bash
    set -euo pipefail
    # A scratch workspace with just these crates and no dev-dependencies: the whole
    # workspace can't resolve at minimal versions (rama pins time =0.3.47).
    members=(crates/vld crates/vld-derive crates/vld-web crates/vld-openapi crates/vld-data crates/vld-app)
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
    cargo +nightly check --locked --manifest-path "$m" -p vld-web --features warp,tonic
    cargo +nightly check --locked --manifest-path "$m" -p vld-openapi --features schemars-0,schemars-1
    cargo +nightly check --locked --manifest-path "$m" -p vld-app --features config-rs,figment
    cargo +nightly check --locked --manifest-path "$m" -p vld-data --features redis-0,redis-1,redis-aio-tokio

# Crates not on crates.io yet are skipped; the baselines' rustdoc builds are removed.
# Public API changes vs crates.io match the version bump (all features)
semver:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'rm -rf target/semver-checks || true' EXIT
    # vld-derive is a proc-macro: no library API for semver-checks to compare.
    for c in vld vld-web vld-openapi vld-data vld-app; do
      if cargo info --registry crates-io "${c}" >/dev/null 2>&1; then
        cargo semver-checks -p "${c}" --all-features
      else
        echo "semver: ${c} is not on crates.io yet, skipped"
      fi
    done

# Per-feature builds leave one artifact set per feature behind; needs cargo-sweep.
# Cap target/ at 10G, oldest artifacts first
trim:
    @if command -v cargo-sweep >/dev/null; then cargo sweep --maxsize 10GB .; else echo "cargo-sweep not installed: target/ not trimmed"; fi

# Remove all build output
clean:
    cargo clean
    rm -rf fuzz/target

# fuzz/ is outside the workspace; run by the release-plz workflow on the release PR.
# Release PR: [Unreleased] -> [version] in CHANGELOG.md, fuzz/Cargo.lock at the new version
release-prep version: (changelog version)
    cargo update --manifest-path fuzz/Cargo.toml --workspace

# The CHANGELOG.md section of a released version (the GitHub release body)
release-notes version:
    #!/usr/bin/env python3
    import re, sys
    from pathlib import Path
    text, version = Path("CHANGELOG.md").read_text(), "{{ version }}"
    m = re.search(rf"^## \[{re.escape(version)}\][^\n]*\n", text, re.M)
    if not m:
        sys.exit(f"CHANGELOG.md has no section for {version}")
    nxt = re.search(r"^## \[", text[m.end():], re.M)
    print(text[m.end(): m.end() + nxt.start() if nxt else None].strip())

[private]
changelog version:
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
