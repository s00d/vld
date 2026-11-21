# Publishing

The six crates (`vld`, `vld-derive`, `vld-web`, `vld-openapi`, `vld-data`,
`vld-app`) share one version and are released together from GitHub Actions
with [release-plz](https://release-plz.ieni.dev). Nothing is published from a
laptop.

## How a release happens

1. **Write the notes.** Commits land on `main` as usual (conventional commit
   messages: `fix:`, `feat:`, `feat!:` …). Keep `## [Unreleased]` at the top of
   `CHANGELOG.md` filled in by hand (Breaking changes / Migration / Added /
   Changed / Fixed / Security / Removed); it is never generated from git.
2. **Release PR.** On every push to `main` the `Release PR` workflow
   (`.github/workflows/release-plz.yml`) opens or updates the PR
   `chore: release vX.Y.Z`:
   - release-plz bumps every crate to the next version (one version for all,
     `release-plz.toml`): `fix` -> patch, `feat` or a breaking change ->
     minor while 0.x;
   - `just release-prep X.Y.Z` renames `## [Unreleased]` to
     `## [X.Y.Z] - date` and updates `fuzz/Cargo.lock`; it fails when
     `[Unreleased]` is empty;
   - `just semver` compares the public API (every feature) with crates.io and
     fails the run when the change is bigger than the bump allows.

   Edit the PR if needed (version, notes). It is updated again on the next
   push to `main`.
3. **Merge the release PR.** CI runs on `main`; when lint, tests (Linux,
   macOS, Windows) and the misc checks pass, the `release` job runs
   `release-plz release`: it publishes the crates whose version is not on
   crates.io yet, in dependency order, tags `vX.Y.Z`, and creates the GitHub
   release with the `CHANGELOG.md` section of that version
   (`just release-notes X.Y.Z`). On any other push to `main` it publishes
   nothing.

Before a release with risky dependency changes, run the manual checks
(Actions -> CI -> Run workflow: MSRV toolchains, minimal versions, feature
pairs, latest stable clippy).

## crates.io authentication

The `release` job uses crates.io
[trusted publishing](https://crates.io/docs/trusted-publishing): no token is
stored anywhere. One-time setup per crate on crates.io (Settings -> Trusted
Publishing): repository `s00d/vld`, workflow `ci.yml`, environment `crates-io`.

crates.io cannot create a crate through trusted publishing: a crate's first
version needs a token. For such a release (0.8.0 introduces `vld-web`,
`vld-openapi`, `vld-data`, `vld-app`), add a repository secret
`CARGO_REGISTRY_TOKEN` (a crates.io token with publish rights) before merging
the release PR; the `release` job uses it when present. Then configure trusted
publishing for the new crates and delete the secret.

## Rate limits

crates.io limits publishing with a token **per user, across all crates**: a
burst of 30 updates of existing crates, then one per minute, and 5 new crate
names, then one per 10 minutes (`src/rate_limiter.rs`, keyed by user id).
Trusted publishing has no user, and crates.io applies no rate limit to it.
A release publishes six crates. If publishing stops half-way (rate limit,
network), re-run the `release` job: release-plz skips what is already on
crates.io.

## New crates

Prefer a module in an existing group crate (ARCHITECTURE.md) over a new
crate. A new crate needs: a `crates/<name>` member in the root `Cargo.toml`,
an entry in `[workspace.dependencies]` if other crates use it, `readme` and
`include = ["/src/**", "/README.md", "/LICENSE", "/Cargo.toml"]` (the leading
`/` anchors the pattern), `[lints] workspace = true`, a `[[package]]` entry
with `version_group = "vld"` in `release-plz.toml`, a row in `MSRV.md`, a
group in the `justfile` and the CI matrix, and a token for its first
release (above).

## Retiring the 0.7 crates (once, after 0.8.0)

0.8 replaced 28 crates (`vld-axum`, `vld-sqlx`, `vld-ts`, …) with modules of
`vld`, `vld-web`, `vld-openapi`, `vld-data` and `vld-app`. Branch
`deprecate-0.7` (from tag `v0.7.0`) holds a 0.7.1 of each: the 0.7.0 code
unchanged, a description starting with `DEPRECATED: moved to …`, and a notice
with the new crate, feature, module and the migration guide at the top of the
README and the crate docs. Publish it once 0.8.0 is on crates.io (the links
point there), from a checkout of that branch, with a token
(`cargo login`; trusted publishing is not set up for these crates):

```sh
git switch deprecate-0.7
# order does not matter: their vld-* dependencies ask for ^0.7.0, already published
for c in $(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.version == "0.7.1") | .name'); do
  cargo info --registry crates-io "${c}@0.7.1" >/dev/null 2>&1 && continue   # already published
  cargo publish -p "${c}"
done
git tag deprecate-0.7.1 && git push origin deprecate-0.7.1
```

That is 28 token publishes from one user: within the burst of 30 if no other
token publish happened in the last half hour (the 0.8.0 release with a token
for the new crates counts: wait 30 minutes after it). If the limit hits,
re-run the loop a minute later; it skips crates already at 0.7.1. Then delete the branch. Nothing else is published from it.

## Troubleshooting

- **Release PR run fails at `release-prep`**: `## [Unreleased]` is missing or
  empty; write the notes on `main`, push.
- **Release PR run fails at `semver`**: the public API changed more than the
  bump allows (a breaking change in a patch). Make the change compatible, or
  mark it breaking (`feat!:`) so the next version is a minor.
- **`release` job failed**: fix the cause and re-run the job; it is
  idempotent. What gets uploaded: `cargo package -p <crate> --list`.
