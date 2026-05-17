# CLAUDE.md — `src/` guide

Library + binary crate for `rprof`. Compiles to one binary (`rprof`) with two
user-facing subcommands. The library is exposed (`pub` modules in `lib.rs`)
so integration tests in `tests/` can reuse types and helpers.

## Module map

| File | Responsibility |
|---|---|
| `main.rs` | Thin shim. Calls `cli::run()` and translates the result into a `ExitCode`. |
| `lib.rs` | Library root. Declares the `pub` module surface (`cli`, `schema`, `proc_parse` [linux-only], `sampler`, `runner`, `viewer`) that integration tests in `tests/` link against. Add new top-level modules here. |
| `cli.rs` | `clap` parsing for `run` and `view` plus a hidden `__alloc-fixture` test helper. Dispatches to `runner` / `viewer`. |
| `schema.rs` | Frozen JSONL schema (v1) with `serde` derives. `Record` is the tagged enum of `Header` / `Sample` / `Footer` rows; `SCHEMA_VERSION` is the version gate the viewer checks. |
| `proc_parse.rs` | Linux-only. Parsers for `/proc/<pid>/stat` (`ProcStat`) and `/proc/<pid>/io` (`ProcIo`), plus the `count_fds()` helper over `/proc/<pid>/fd/`. String-in / struct-out so fixtures drive the parser tests. |
| `sampler.rs` | `Sampler` trait + Linux `ProcSampler` backend that wraps `proc_parse`. Returns `Ok(None)` when the target is gone. |
| `runner.rs` | `rprof run`: spawn child, install signal forwarder, stream a header + per-tick samples + footer to disk. |
| `viewer.rs` | `rprof view`: load reports (line-by-line, tolerant of unknown record types and a truncated trailing line), derive per-sample CPU% and peak RSS, render the self-contained HTML. Embeds `assets/*` via `include_str!`. |

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
- CPU% is computed by the reader (viewer) from the cumulative
  `utime_ticks` / `stime_ticks` carried on each `sample` record, using
  `host.clock_ticks_per_sec` from the report's header. The runner does
  **not** derive CPU% — it only records raw ticks. One pegged core reads
  as 100%, four pegged cores as 400%. Do not normalise to total cores;
  users expect top-style numbers.

### Signal handling

- `runner.rs` installs handlers for SIGINT/SIGTERM/SIGHUP via `libc::signal`
  with a tiny async-signal-safe forwarder (atomic load + `libc::kill`).
- The header and any sample records already on disk survive any
  catchable signal — they were flushed each tick. The footer must also
  be written before `rprof` exits when the child died from a forwarded
  (catchable) signal. The integration test
  `run_forwards_sigint_and_still_writes_report` protects this.
- SIGKILL is uncatchable; the file in that case is a partial report
  (header + samples, no footer). The
  `run_killed_with_sigkill_leaves_header_and_samples_no_footer` test
  pins that contract.

### Schema (`schema.rs`)

- `SCHEMA_VERSION` is frozen at `1`. Bump only on a breaking change.
- The on-disk format is JSON Lines: one `Record` per line. `Record`
  is a serde-tagged enum (`type = "header" | "sample" | "footer"`).
- Adding new fields is additive and **does not** bump the version; viewer
  and writer use serde's permissive defaults. The unit test
  `additive_fields_tolerated_on_read` pins this for reads.
- Defining a new record type does not bump the version either; readers
  ignore unknown record types. `unknown_record_type_is_skipped_by_reader`
  pins the deserialiser side; `parse_jsonl_tolerates_unknown_record_types`
  pins it through the viewer loader.
- Per-sample (one record per timestamp) layout is mandatory: it is what
  makes streaming writes and partial-file recovery possible. Do not
  convert to columnar.

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
