# Publishing

The six crates (`vld`, `vld-derive`, `vld-web`, `vld-openapi`, `vld-data`,
`vld-app`) share one version and are released from GitHub Actions. Nothing is
published from a laptop and no crates.io token is stored anywhere.

## Release

1. Keep `## [Unreleased]` at the top of `CHANGELOG.md` filled in by hand
   (Breaking changes / Migration / Added / Changed / Fixed / Security /
   Removed). It is never generated from git.
2. Wait for green CI on `main`.
3. Actions -> **CI** -> **Run workflow** -> `release`: the version (e.g.
   `0.8.0`, no `v`) -> Run. The `release` job:
   - stops unless CI is green on `main` HEAD, or on an earlier commit with only
     docs, `*.md` files or the justfile changed since (those need no new CI run);
   - `just release-prep X.Y.Z`: sets every crate's version, updates the
     lockfiles, renames `[Unreleased]` to `[X.Y.Z] - date` (stops when it is
     empty);
   - `just semver`: compares the public API (every feature) with crates.io and
     stops when the change is bigger than the version allows;
   - commits `chore: release vX.Y.Z` to `main` and tags `vX.Y.Z`;
   - publishes the crates in dependency order with crates.io trusted
     publishing;
   - creates the GitHub release with the `CHANGELOG.md` section
     (`just release-notes X.Y.Z`).

   If it fails after tagging (network, crates.io), run it again with the same
   version: it publishes only what is missing and creates the GitHub release
   if needed.

Run workflow with an empty `release` field runs the extended checks instead
(MSRV toolchains, minimal versions, feature pairs, latest stable clippy);
worth doing before a release with risky dependency changes.

## crates.io trusted publishing

Each crate has a trusted publisher on crates.io (Settings -> Trusted
Publishing): owner `s00d`, repository `vld`, workflow `ci.yml`, environment
`crates-io`. crates.io cannot create a crate through trusted publishing: a new
crate gets a placeholder `0.0.0` published by hand first (`cargo publish` of an
empty crate with the same name), then its trusted publisher, then the release
publishes the real version.

## Rate limits

crates.io limits token publishes **per user, across all crates** (a burst of
30 updates, then one per minute; 5 new crates, then one per 10 minutes;
`src/rate_limiter.rs`). Trusted publishing has no user and no rate limit.

## New crates

Prefer a module in an existing group crate (ARCHITECTURE.md) over a new
crate. A new crate needs: a `crates/<name>` member in the root `Cargo.toml`,
an entry in `[workspace.dependencies]` if other crates use it, `readme` and
`include = ["/src/**", "/README.md", "/LICENSE", "/Cargo.toml"]` (the leading
`/` anchors the pattern), `[lints] workspace = true`,
a row in `MSRV.md`, a group in the `justfile` and the CI matrix, the crate in
the publish loop of the CI `release` job, and a placeholder plus trusted
publisher (above).

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

- **`release` stops at "CI on main ... is not success"**: wait for the CI run
  of the last push to `main`, or fix it.
- **Stops at `release-prep`**: `## [Unreleased]` is missing or empty.
- **Stops at `semver`**: the API changed more than the version allows (a
  breaking change in a patch); release a minor instead.
- **Failed after the tag**: run it again with the same version.
