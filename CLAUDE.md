# CLAUDE.md — rprof project guide

The minimum context to work on this repository without re-discovering it
every session. Read the routing table before modifying anything; the
per-directory `CLAUDE.md` files hold the constraints specific to each
area.

## What rprof is

`rprof` is a process-resource profiler: it runs a command, samples the
child's CPU / memory / IO from `/proc` on a background thread, and
renders the result as a self-contained HTML report. Two subcommands:

| Subcommand | Purpose |
|---|---|
| `rprof run -- <cmd>` | Spawn `<cmd>`, poll `/proc/<pid>`, stream a versioned JSONL report to disk as samples are taken. |
| `rprof view <r.jsonl> [<r.jsonl> ...]` | Render one or more reports as a self-contained HTML file with interactive uPlot charts. |

The project's goals and deliberate non-goals — what rprof will *not*
become — are recorded in
[`docs/rationale/project-scope.md`](docs/rationale/project-scope.md). A
feature that touches a non-goal needs a decision there first.

## Routing — read before modifying

| When you are about to... | Read first |
|---|---|
| Find project documentation, scope, or a how-to guide | [`docs/CLAUDE.md`](docs/CLAUDE.md) |
| Understand how data flows end to end | [`docs/architecture.md`](docs/architecture.md) |
| Add or change a CLI flag, subcommand, or runner/viewer flow | [`src/CLAUDE.md`](src/CLAUDE.md) |
| Add or modify a unit or integration test | [`tests/CLAUDE.md`](tests/CLAUDE.md) |
| Touch vendored uPlot files or the viewer JS/CSS | [`assets/CLAUDE.md`](assets/CLAUDE.md) |
| Add, change, or check a functional requirement | [`docs/requirements/CLAUDE.md`](docs/requirements/CLAUDE.md) |
| Record or look up a design decision (or a rejected option) | [`docs/rationale/CLAUDE.md`](docs/rationale/CLAUDE.md) |
| Dogfood the tool or iterate on the viewer interactively | [`scripts/CLAUDE.md`](scripts/CLAUDE.md) |

Do not skip these reads. They contain area-specific rules (e.g. "do not
edit vendored uPlot files in place") that prevent regressions.

## Contributing

### Pre-commit checks (mandatory)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass before committing. Fix the underlying issue; never
bypass hooks. The first `cargo test` after a checkout is slower because
the integration tests build the `rprof` binary on demand.

Build with `cargo build` (debug) or `cargo build --release` (LTO,
stripped, single static binary, ~1.3 MB).

### New features and behaviour changes

Requirement-first. Check [`docs/requirements/`](docs/requirements/) for a
spec; if none exists, write a `proposed` one before coding, implement it
TDD-style against tests that cite the requirement, then flip it to
`implemented` when they pass. The full protocol — statuses, test tags,
the coverage check — is in
[`docs/requirements/CLAUDE.md`](docs/requirements/CLAUDE.md). A bug fix
that does not change a contract skips straight to test + fix, citing the
requirement if one exists.

### Commit messages

- Imperative subject under 70 characters, with an area prefix where it
  sharpens the scan-line (`docs:`, `fix:`, `test:`, `feat:`, …).
- Blank line, then a body explaining the *why*; the diff shows the
  *what*. No trailing change-summary.
- Reference a requirement ID when the commit implements or modifies that
  contract; reference an issue or PR only when it adds context the body
  can't.

### Code style

- Rust 2021 edition. MSRV `1.75` (see `Cargo.toml`).
- Prefer editing existing files over adding modules. No speculative
  abstractions, no error handling for impossible cases.
- Comments explain *why*, not *what*; default to none unless a subtle
  invariant would otherwise need re-deriving. The SPDX file-header rule
  is in [`src/CLAUDE.md`](src/CLAUDE.md).

## Licensing

MIT (see `LICENSE`). Bundled assets are covered in
[`assets/CLAUDE.md`](assets/CLAUDE.md) — currently only uPlot 1.6.31
(MIT) is vendored.
