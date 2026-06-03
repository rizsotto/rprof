---
paths:
  - "**/*.rs"
---

# Test conventions

Where tests live and how to write them. The per-directory `tests/CLAUDE.md`
adds the integration-test layout and harness specifics on top of these.

- **Placement**: pure functions (parsers, derivations) get unit tests in
  an in-file `#[cfg(test)] mod tests` at the bottom of the `.rs` file,
  driven by fixture strings — that is why the parsers take `&str` / `&[u8]`
  rather than opening files themselves. Behavioural contracts get
  integration tests under `tests/`, each tagged `// Requirements: <id>`
  immediately above the `#[test]` (see `docs/requirements/CLAUDE.md` for
  the linkage rules and `tests/CLAUDE.md` for the harness).
- **One scenario per `#[test]`.** Give each test a name that states the
  behaviour it pins (`parse_proc_stat_handles_comm_with_spaces`). Set up
  the inputs, produce the value under test, then assert on it.
- **Fixtures are data, kept close.** Inline a `const` fixture string next
  to the tests that use it, with a comment explaining what the bytes
  represent. For on-disk scaffolding use `tempfile::tempdir()` and let it
  clean up; never write into the repo root or `./.rprof/`.
- **Workloads come from `examples/`.** The `alloc_fixture` example
  (`examples/alloc_fixture.rs`) is the canonical way to produce a process
  with a known RSS footprint; the integration helper `alloc_fixture_bin()`
  builds it on demand. Reach for it instead of `dd`/`python`/etc. The
  shipped binary exposes only `run` and `view` — no hidden test-only
  subcommands.
- **Spawn the CLI via `env!("CARGO_BIN_EXE_rprof")`,** never `cargo run`.
- **Keep the suite fast and deterministic.** Avoid sleeps longer than a
  second; the whole suite should finish in a couple of seconds. Flaky
  timing is the most common failure mode — run all of `cargo test`, not
  just the new test, before committing.
