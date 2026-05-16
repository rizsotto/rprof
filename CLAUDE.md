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
| `rprof run -- <cmd>` | Spawn `<cmd>`, poll `/proc/<pid>` on a background thread, write a versioned JSON report on exit. |
| `rprof view <r.json> [<r.json> ...]` | Render one or more reports as a self-contained HTML file with interactive uPlot charts. |

Linux-first; macOS and the cgroup-v2 backend are deferred to later phases.
See [`idea.md`](idea.md) for the full design and roadmap.

## Routing — read before modifying

| When you are about to... | Read first |
|---|---|
| Add or change a CLI flag, subcommand, or runner/viewer flow | [`src/CLAUDE.md`](src/CLAUDE.md) |
| Add or modify a unit or integration test | [`tests/CLAUDE.md`](tests/CLAUDE.md) |
| Touch vendored uPlot files or the viewer JS/CSS | [`assets/CLAUDE.md`](assets/CLAUDE.md) |
| Add, change, or check a functional requirement | [`requirements/CLAUDE.md`](requirements/CLAUDE.md) |

Do not skip these reads. They contain area-specific rules (e.g. "do not edit
vendored uPlot files in place") that prevent regressions.

## Architecture (data flow)

```
rprof run -- cmd
   |
   v
spawn child (stdio inherited)  ──>  signal forwarder (SIGINT/SIGTERM/SIGHUP)
   |
   v
sampler thread polls /proc/<pid>  every --interval ms
   |
   v
child exits  ──>  wait(), getrusage(RUSAGE_CHILDREN)
   |
   v
build Report, serialize JSON, write to --output (or .rprof/<ts>.json)
```

```
rprof view r1.json r2.json ...
   |
   v
parse + version-check reports
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

1. Check [`requirements/`](requirements/) for an existing spec.
2. If absent, write a `proposed` requirement file before coding (see
   [`requirements/CLAUDE.md`](requirements/CLAUDE.md) for the template).
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
- Every Rust source and test file starts with `// SPDX-License-Identifier: MIT`.

## Licensing

- Project: MIT (see `LICENSE`).
- Bundled assets: see [`assets/CLAUDE.md`](assets/CLAUDE.md). Currently only
  uPlot 1.6.31 (MIT) is vendored.
