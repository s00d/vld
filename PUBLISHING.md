# Publishing

All crates share one version (`[workspace.package] version`) and are
released together with [cargo-release](https://github.com/crate-ci/cargo-release).

## One-time setup

```sh
cargo install cargo-release just cargo-hack cargo-deny --locked
cargo login            # crates.io token with publish rights for every vld crate
gh auth login          # for the GitHub release
```

## Release

1. Add `## [Unreleased]` at the top of `CHANGELOG.md` and write the notes under it (Breaking
   changes / Migration / Added / Changed / Fixed / Security / Removed). The
   release stops if that section is empty: it is never regenerated from git.
2. Make sure `main` is green in CI, and run the weekly checks once by hand
   (Actions → CI → Run workflow: MSRV, minimal versions, Windows / macOS).
3. Dry run, then release (`minor` for 0.x breaking releases, `patch` for fixes):

   ```sh
   cargo release minor --workspace             # dry run
   cargo release minor --workspace --execute
   ```

   cargo-release bumps every `Cargo.toml` (internal dependencies use
   `workspace = true`, so `[workspace.dependencies]` is the one place with
   versions), then runs the pre-release hook once (`just release-hook`):
   `just fmt lint test docs package`, renames `[Unreleased]` to
   `[X.Y.Z] - date` in `CHANGELOG.md` and updates `fuzz/Cargo.lock`.

   It then commits `chore: release vX.Y.Z`, publishes every crate **in
   dependency order** (computed by cargo-release; no manual batches), tags
   `vX.Y.Z` and pushes the commit and the tag.

4. Create the GitHub release (always, a tag alone is not enough):

   ```sh
   gh release create vX.Y.Z --title "vX.Y.Z" --latest --notes "<the X.Y.Z section of CHANGELOG.md>"
   ```

## New crates

crates.io allows only a few **new** crate names in a short burst (about 5,
then roughly one per 10 minutes). A release that adds more than that will fail
part-way: wait, then rerun `cargo release publish --workspace --execute`; it
skips what is already published. 0.5.0 adds one new crate,
`vld-integration-common`, which is published before its dependents.

A new crate also needs: a `crates/<name>` member in the root `Cargo.toml`, an
entry in `[workspace.dependencies]` if other crates use it,
`include = ["/src/**", "/README.md", "/LICENSE", "/Cargo.toml"]` (the leading
`/` anchors the pattern: without it `README.md` also matches every README in
`docs/node_modules`), a row in `MSRV.md`, and an entry in `feature_crates` in
the `justfile` if it has optional features.

## Troubleshooting

- **`[Unreleased]` is missing or empty**: add the heading and the notes, commit, rerun.
- **Hook checks fail**: fix on `main`, rerun the release (nothing was
  published yet; cargo-release has not committed).
- **Publish stopped half-way** (rate limit, network, index lag): rerun
  `cargo release publish --workspace --execute`, then
  `cargo release tag --workspace --execute` and
  `cargo release push --workspace --execute` if those steps did not run.
- **What gets uploaded**: `cargo package -p <crate> --list`.
- **Wrong files / dirty tree**: cargo-release refuses to run with
  uncommitted changes; commit or stash first.
