---
name: release
description: Cut a release of rprof — bump the crate version, run the full check set, tag, and publish a GitHub release. Use when the user asks to release, cut a version, tag a release, or ship a new rprof version.
---

# Release rprof

Cut a release of the `rprof` crate. The process is manual today: there is
no `cargo-dist`, no release workflow, and no `CHANGELOG` — only the CI
lint + test job. Drive the steps below, and **confirm with the user
before any outward-facing action** (pushing a tag, creating a GitHub
release, publishing to crates.io). None of those can be cleanly undone.

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
6. **Push** (confirm first): `git push origin master --tags`.
7. **GitHub release** (confirm first): create a release for the tag with
   `gh release create v<X.Y.Z>`. Draft notes from
   `git log <previous-tag>..v<X.Y.Z>` (or the full log for the first
   release). Attach the `target/release/rprof` binary from step 3 if a
   downloadable artefact is wanted.

## Optional: publish to crates.io

Only if the user asks to publish there. Run `cargo publish --dry-run`
first and show the output; a real `cargo publish` cannot be undone, only
yanked. It needs an unpublished version and the manifest metadata
crates.io requires (`description`, `license`, `repository`).

## Notes

- Prebuilt cross-platform binaries, a Homebrew tap, and a `CHANGELOG`
  are not yet set up; they are tracked in the issue tracker, not under
  `docs/requirements/` (see `docs/project-scope.md`). If one lands later,
  update the corresponding step here.
