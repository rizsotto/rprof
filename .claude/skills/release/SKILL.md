---
name: release
description: Cut a release of rprof — bump the crate version, run the full check set, tag, and publish a GitHub release. Use when the user asks to release, cut a version, tag a release, or ship a new rprof version.
---

# Release rprof

Cut a release of the `rprof` crate. There is no `cargo-dist` and no
`CHANGELOG`, but publishing **is** automated: pushing a `v*` tag triggers
`.github/workflows/release.yml`, which re-runs the check set and then
`cargo publish`es to crates.io. So the human gate is the *tag push* —
once a `v*` tag reaches GitHub, the crate is published with no further
confirmation, and that cannot be cleanly undone (only yanked). Drive the
steps below and **confirm with the user before any outward-facing
action** (pushing the tag, creating a GitHub release).

## Before you start

- Confirm you are on `master` with a clean working tree (`git status`).
  If not, stop and ask.
- Ask the user for the target version (e.g. `0.2.0`) if they did not give
  one. rprof is pre-1.0; follow semver. A bumped MSRV (`rust-version` in
  `Cargo.toml`) is itself semver-relevant — surface it in the notes.
- The report schema has its own version, `SCHEMA_VERSION` in
  `src/schema.rs`, frozen at `1`. It is bumped only on a *breaking*
  schema change, independently of the crate version — leave it alone
  unless the release contains one.

## Steps

1. **Green master.** Run and require all to pass:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   sh scripts/check-requirements-coverage.sh
   ```
   If anything fails, stop — do not release on a red baseline.
2. **Bump the version** in `Cargo.toml`, then run `cargo build` so
   `Cargo.lock` updates in the same change.
3. **Verify the release artefact:** `cargo build --release`, confirm the
   binary is a single static file (~1 MB) and that
   `./target/release/rprof run -- true` writes a report.
4. **Commit** on `master`: `chore: release v<X.Y.Z>`.
5. **Tag** (confirm first): `git tag -a v<X.Y.Z> -m "rprof v<X.Y.Z>"`.
6. **Push** (confirm first — this is the point of no return). Push
   `master` first, then the tag:
   ```bash
   git push origin master
   git push origin v<X.Y.Z>
   ```
   Pushing the tag triggers `.github/workflows/release.yml`, which gates
   on the check set and then publishes to crates.io automatically. Watch
   the run (`gh run watch`) and confirm it goes green before announcing
   the release.
7. **GitHub release** (confirm first): create a release for the tag with
   `gh release create v<X.Y.Z>`. Draft notes from
   `git log <previous-tag>..v<X.Y.Z>` (or the full log for the first
   release). Attach the `target/release/rprof` binary from step 3 if a
   downloadable artefact is wanted.

## crates.io publishing

Handled automatically by the release workflow on tag push (step 6); you
do not run `cargo publish` by hand. Two preconditions the workflow can't
fix for you:

- The repo secret `CRATES_TOKEN` must be set (GitHub → Settings →
  Secrets and variables → Actions) to a crates.io API token with
  publish scope.
- The version in `Cargo.toml` must be unpublished — crates.io rejects a
  re-publish, so the bump in step 2 is mandatory.

To sanity-check before tagging, run `cargo publish --dry-run --locked`
locally and show the output. The manifest already carries the metadata
crates.io requires (`description`, `license`, `repository`).

## Notes

- Prebuilt cross-platform binaries, a Homebrew tap, and a `CHANGELOG`
  are not yet set up; they are tracked in the issue tracker, not under
  `docs/requirements/` (see `docs/project-scope.md`). If one lands later,
  update the corresponding step here.
- The publish workflow lives in `.github/workflows/release.yml`. If its
  trigger or steps change (e.g. the check set, or the tag glob), keep
  step 6 above in sync.
