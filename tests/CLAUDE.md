# CLAUDE.md — `tests/` guide

Integration tests for `rprof`. Each `*.rs` file under `tests/` compiles to a
separate test binary that links against the `rprof` library and spawns the
`rprof` CLI itself via `env!("CARGO_BIN_EXE_rprof")`.

## Layout

| File | Scope |
|---|---|
| `runner_integration.rs` | End-to-end tests for `rprof run`: spawn, exit-code, signals, output paths, peak-RSS accuracy. |
| `viewer_integration.rs` | End-to-end tests for `rprof view`: stdout, file output, overlay mode, schema rejection. |

Unit tests live next to the code they exercise (`#[cfg(test)] mod tests` at
the bottom of each `src/*.rs`).

## Conventions

- Every test file is gated with `#![cfg(target_os = "linux")]` because the
  runner requires `/proc`. File-header rules (SPDX) are documented once in
  [`../src/CLAUDE.md`](../src/CLAUDE.md) and apply equally here.
- Tests must clean up after themselves. Use `tempfile::tempdir()` for any
  files; never write into the repo root or `./.rprof/` (except the one test
  that exercises the auto-output path, which uses `current_dir()` on a
  tempdir).
- The hidden `__alloc-fixture` subcommand of `rprof` is the canonical way to
  produce a workload with a known RSS footprint. Reach for it instead of
  `dd`/`python`/etc.
- Spawn `rprof` via `env!("CARGO_BIN_EXE_rprof")`, never via `cargo run`.
- Avoid sleeps longer than 1 second. The whole suite should finish in a
  couple of seconds on a developer laptop.

## Requirements traceability

When a test protects a specific behavioural requirement in
[`requirements/`](../requirements/), annotate it with a `Requirements:`
comment immediately above the `#[test]` attribute:

```rust
// Requirements: capture-signal-forwarding
#[test]
fn run_forwards_sigint_and_still_writes_report() { ... }
```

Multiple requirements are comma-separated. The annotation is plain prose, not
a macro — `cargo test` ignores it but grep finds it:

```bash
grep -rn "Requirements:.*capture-signal-forwarding" tests/ src/
```

See [`requirements/CLAUDE.md`](../requirements/CLAUDE.md) for the canonical
test ↔ requirement linkage rules.

## Adding a new integration test

1. Decide which existing file it belongs in (or, if a genuinely new area,
   create a new `*_integration.rs`).
2. Tag it with the requirement ID(s) it protects.
3. Run the full suite (`cargo test`) — not just the new test — before
   committing. Flaky timing is the most common failure mode.
