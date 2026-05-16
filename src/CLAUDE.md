# CLAUDE.md — `src/` guide

Library + binary crate for `rprof`. Compiles to one binary (`rprof`) with two
user-facing subcommands. The library is exposed (`pub` modules in `lib.rs`)
so integration tests in `tests/` can reuse types and helpers.

## Module map

| File | Responsibility |
|---|---|
| `main.rs` | Thin shim. Calls `cli::run()` and translates the result into a `ExitCode`. |
| `cli.rs` | `clap` parsing for `run` and `view` plus a hidden `__alloc-fixture` test helper. Dispatches to `runner` / `viewer`. |
| `schema.rs` | Frozen JSON schema (v1) with `serde` derives. `SCHEMA_VERSION` is the version gate the viewer checks. |
| `proc_parse.rs` | Linux-only. Parsers for `/proc/<pid>/stat` (`ProcStat`) and `/proc/<pid>/io` (`ProcIo`), plus helpers `count_fds()` over `/proc/<pid>/fd/` and `read_children()` over `/proc/<pid>/task/*/children`. String-in / struct-out so fixtures drive the parser tests. |
| `sampler.rs` | `Sampler` trait + Linux `ProcSampler` backend that wraps `proc_parse`. Returns `Ok(None)` when the target is gone. |
| `runner.rs` | `rprof run`: spawn child, install signal forwarder, sample on a thread, compute CPU% from tick deltas, serialise and write the JSON. |
| `viewer.rs` | `rprof view`: load reports, render the self-contained HTML, write or open. Embeds `assets/*` via `include_str!`. |

## Rules

### CLI invariants

- The `--` separator on `rprof run` is mandatory. Everything after it is the
  child command, forwarded verbatim.
- Exit code mirrors the child (or `128 + signum` if the child died from a
  signal). `rprof run` must remain drop-in compatible with shell pipelines.
- Hidden subcommands (`__alloc-fixture`) are test fixtures. Hide them with
  `hide = true` and document them as test-only in the docstring.

### Sampling

- `Sampler::sample()` returns `Ok(None)` when the target PID is gone. The
  polling loop uses that as the graceful stop signal — do not change it to
  an error.
- All `/proc` reads must tolerate `ENOENT` mid-sample (entries can disappear
  between `readdir` and `open`).
- CPU% is `delta_ticks / sysconf(_SC_CLK_TCK) / dt_seconds * 100`. One pegged
  core reads as 100%, four pegged cores as 400%. Do not normalise to total
  cores; users expect top-style numbers.

### Signal handling

- `runner.rs` installs handlers for SIGINT/SIGTERM/SIGHUP via `libc::signal`
  with a tiny async-signal-safe forwarder (atomic load + `libc::kill`).
- The JSON report must always be written before `rprof` exits, even when the
  child died from a forwarded signal. This is an acceptance criterion; the
  integration test `run_forwards_sigint_and_still_writes_report` protects it.

### Schema (`schema.rs`)

- `SCHEMA_VERSION` is frozen at `1`. Bump only on a breaking change.
- Adding new fields is additive and **does not** bump the major; viewer + writer
  use serde's permissive defaults. The unit test
  `additive_fields_tolerated_on_read` enforces this for reads.
- Per-sample (one object per timestamp) layout is intentional: keeps in-memory
  use low during capture. Do not convert to columnar without a strong reason.

### Viewer (`viewer.rs`)

- HTML output is a single self-contained file. Do not introduce external
  references (no CDNs, no separate JSON files, no asset directories).
- The inlined JSON payload must escape `</` to `<\/` to survive embedding
  inside `<script type="application/json">`. `payload_escapes_closing_script_tags`
  pins this.
- `render_html()` is `pub` so tests can call it directly; keep it free of side
  effects (no fs writes, no `Command` execution).

## File headers

Every `.rs` file under `src/` (and `tests/`) starts with
`// SPDX-License-Identifier: MIT` as the first line, before module docs.
