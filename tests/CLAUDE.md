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

The general test house style is in
[`../.claude/rules/testing.md`](../.claude/rules/testing.md) (and the Rust
house style in [`../.claude/rules/rust.md`](../.claude/rules/rust.md)),
applied automatically when you edit any `.rs` file. The harness specifics
for *this* directory:

- Every test file is gated whole-file with `#![cfg(target_os = "linux")]`
  because the runner requires `/proc`.
- The one exception to the "never write into the repo root or `./.rprof/`"
  rule is the test that exercises the auto-output path, which uses
  `current_dir()` on a tempdir.

## Requirements traceability

When a test protects a specific behavioural requirement in
[`docs/requirements/`](../docs/requirements/), annotate it with a `Requirements:`
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

See [`docs/requirements/CLAUDE.md`](../docs/requirements/CLAUDE.md) for the canonical
test ↔ requirement linkage rules.

## Adding a new integration test

1. Decide which existing file it belongs in (or, if a genuinely new area,
   create a new `*_integration.rs`).
2. Tag it with the requirement ID(s) it protects.
3. Run the full suite (`cargo test`) — not just the new test — before
   committing. Flaky timing is the most common failure mode.
