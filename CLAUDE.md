# CLAUDE.md — rprof project guide

This file gives Claude Code agents the minimum context to work on this
repository without re-discovering it every session. Read the routing table
below before modifying anything; the per-directory `CLAUDE.md` files have
the constraints specific to that area.

## Pre-commit checks (mandatory)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass before committing. Fix the underlying issue; never bypass
hooks. Integration tests build the `rprof` binary on demand, so the first
`cargo test` after a checkout takes longer.

### Commit messages

- Imperative subject under 70 characters. An area prefix is encouraged
  (e.g. `docs:`, `ci:`, `deps:`, `feat:`, `fix:`, `test:`) but the list
  is not exhaustive — pick whatever makes the scan-line clear.
- Blank line, then a wrapped body explaining the *why*. The diff
  shows the *what*.
- Reference a requirement ID (e.g. `capture-signal-forwarding`) when
  the commit implements or modifies that contract.
- Reference an issue or PR only if it adds context the body can't.
- No trailing summary of changes — the diff is authoritative.

## Build

```bash
cargo build              # debug
cargo build --release    # release (LTO, stripped, ~1.3 MB)
```

The release binary is a single static executable with no runtime deps; the
viewer HTML is self-contained (uPlot bundled at compile time via
`include_str!`).

## Project overview

`rprof` is a process-resource profiler split into two subcommands:

| Subcommand | Purpose |
|---|---|
| `rprof run -- <cmd>` | Spawn `<cmd>`, poll `/proc/<pid>` on a background thread, stream a versioned JSONL report to disk as samples are taken. |
| `rprof view <r.jsonl> [<r.jsonl> ...]` | Render one or more reports as a self-contained HTML file with interactive uPlot charts. |

### Project goals

- Single static binary with no runtime dependencies. Drop it into a
  container or CI image and it works.
- Capture is non-invasive: no instrumentation, no `LD_PRELOAD`, no
  `ptrace`. The target program is unaware it is being measured.
- Output is plain JSON Lines with a versioned schema. Trivially
  scriptable, diffable, archivable; `tail -f` works while a capture
  is in progress and `kill -9` leaves a partial-but-usable file
  rather than nothing.
- Visualisation is a self-contained HTML file. No long-running server, no
  port conflicts, no Python or Node required to view a report.
- Diffing multiple runs is a first-class feature, not an afterthought.

### Non-goals (v1)

These are explicitly **out of scope**. New requirements that touch them
need a separate decision rather than a quick "while we're at it":

- Flamegraphs and stack sampling. These need a different acquisition path
  (`perf`, eBPF, DTrace) and a different visualisation; future work
  belongs in a separate subcommand.
- Distributed tracing, network IO breakdown, GPU metrics,
  per-syscall accounting.
- Windows support. Linux is the primary target; macOS is best-effort.
- A long-running daemon or system-wide monitor. `rprof` measures one
  command invocation per process.
- Process-tree resource aggregation. `rprof` reports the resource usage
  of the single child process it spawned. Build wrappers, shells that
  `exec` a tool, or workloads whose interesting work lives in
  grandchildren are not in scope: summing across the tree double-counts
  shared memory and FDs, hides which descendant drove a spike, and
  conflicts with the "one process, one chart" mental model the viewer
  is built around. Users who genuinely need tree-wide accounting
  should reach for cgroup-level tools (`systemd-run --scope`, `cgexec`,
  or `perf stat`); `/usr/bin/time -v` is also single-process and not a
  substitute.

Post-v1 *behavioural* plans are tracked as individual requirement files
with `status: planned` in [`docs/requirements/`](docs/requirements/) —
search the directory for the current backlog.

Distribution and release engineering (prebuilt artefacts via `cargo
dist`, a Homebrew tap, a `SCHEMA.md` cheat sheet, `CONTRIBUTING.md`)
are intentionally **not** tracked under `docs/requirements/`. That directory
captures what the software does, not how it is shipped or documented.
Those tasks belong to release planning and can live in the issue
tracker when picked up.

## Routing — read before modifying

| When you are about to... | Read first |
|---|---|
| Add or change a CLI flag, subcommand, or runner/viewer flow | [`src/CLAUDE.md`](src/CLAUDE.md) |
| Add or modify a unit or integration test | [`tests/CLAUDE.md`](tests/CLAUDE.md) |
| Touch vendored uPlot files or the viewer JS/CSS | [`assets/CLAUDE.md`](assets/CLAUDE.md) |
| Add, change, or check a functional requirement | [`docs/requirements/CLAUDE.md`](docs/requirements/CLAUDE.md) |
| Record or look up a design decision (or a rejected option) | [`docs/rationale/CLAUDE.md`](docs/rationale/CLAUDE.md) |
| Dogfood the tool or iterate on the viewer interactively | [`scripts/CLAUDE.md`](scripts/CLAUDE.md) |

Do not skip these reads. They contain area-specific rules (e.g. "do not edit
vendored uPlot files in place") that prevent regressions.

## Architecture (data flow)

```
rprof run -- cmd
   |
   v
open report file                                  (.rprof/<ts>.jsonl by default)
   |
   v
spawn child (stdio inherited)  ──>  signal forwarder (SIGINT/SIGTERM/SIGHUP)
   |
   v
write `header` record
   |
   v
sampler thread polls /proc/<pid> every --interval ms
   |   each tick: append a `sample` record, flush
   v
child exits  ──>  wait(), getrusage(RUSAGE_CHILDREN)
   |
   v
append `footer` record, flush, close
```

```
rprof view r1.jsonl r2.jsonl ...
   |
   v
parse line-by-line; verify header.schema; tolerate unknown / truncated rows
   |
   v
derive per-sample CPU% and peak RSS in-memory
   |
   v
inline data + uPlot JS/CSS + viewer JS/CSS into single HTML
   |
   v
--no-open + no -o   ──>  HTML to stdout
--no-open + -o P    ──>  HTML written to P
no --no-open        ──>  HTML to -o or temp file, then xdg-open / open
```

## Decision protocol

For new features or behaviour changes:

1. Check [`docs/requirements/`](docs/requirements/) for an existing spec.
2. If absent, write a `proposed` requirement file before coding (see
   [`docs/requirements/CLAUDE.md`](docs/requirements/CLAUDE.md) for the template).
3. Implement TDD-style: write integration tests that reference the
   requirement, then make them pass.
4. Mark the requirement `implemented` once tests are green.

For bug fixes that don't change the contract, jump straight to the test +
fix, but cite the requirement in the test if one exists.

## Code style

- Rust 2021 edition. MSRV: `rust-version = "1.75"` (see `Cargo.toml`).
- Prefer editing existing files over adding new modules.
- No speculative abstractions, no error handling for impossible cases.
- Comments explain *why*, not *what*. Default to none unless the reader
  would otherwise need to re-derive a subtle invariant.
- File-header rules (SPDX) live in [`src/CLAUDE.md`](src/CLAUDE.md).

## Licensing

- Project: MIT (see `LICENSE`).
- Bundled assets: see [`assets/CLAUDE.md`](assets/CLAUDE.md). Currently only
  uPlot 1.6.31 (MIT) is vendored.
