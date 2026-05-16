---
title: Report output path resolution
status: implemented
---

## Intent

Most users want to control where the JSON report lands; for ad-hoc runs they
just want it to land *somewhere* without picking a name. Passing `-o` is
the explicit form; omitting it auto-generates a path under `./.rprof/`
keyed by start time so successive runs do not clobber each other.

## Acceptance criteria

- With `-o <path>`, `rprof` writes the report to exactly `<path>`. If
  the parent directory does not exist, `rprof` creates it.
- Without `-o`, `rprof` writes the report to
  `./.rprof/<YYYY-MM-DDTHHMMSS>.json` (UTC) in the working directory.
  `rprof` creates the `.rprof/` directory if needed.
- If `-o`'s parent directory cannot be created (permissions), `rprof`
  fails with a clear error after the child has exited.
- The report is written atomically *enough* for the v1 contract: a
  single `std::fs::write` call. Readers that race the writer may see a
  partial file, but the failure-on-exit case is covered by
  `capture-signal-forwarding` (the file is written before exit).

## Non-functional constraints

- The auto-generated filename is sortable lexicographically by time so
  shell globbing in chronological order works (`.rprof/*.json | sort`).
- The timestamp format is filesystem-safe on Linux and macOS (no colons,
  no slashes). Windows compatibility is not required for v1.

## Implementation details

- `resolve_output_path()` in `src/runner.rs` picks the explicit `-o`
  value or constructs the timestamped fallback.
- The chrono format string is `%Y-%m-%dT%H%M%S`. The `T` separator
  matches RFC 3339 conventions; the trailing `:` segments are omitted
  to keep the name filesystem-safe.

## Known limitations

- Atomic-rename semantics (write to `.tmp`, then `rename(2)`) are not
  implemented. A reader that opens the file concurrently with the
  writer might see a truncated file. v1 reports are written once at
  process exit, so the window is small.
- The fallback path is always relative to `rprof`'s current working
  directory. There is no `RPROF_OUTPUT_DIR` env var override yet.

## Testing

Given an explicit output path:

> When the user runs `rprof run -o /tmp/out/r.json -- true`,
> then `/tmp/out/` is created if absent,
> and `/tmp/out/r.json` contains the JSON report.

Given no output flag:

> When the user runs `rprof run -- true` inside an empty directory,
> then a `.rprof/` subdirectory is created,
> and it contains exactly one `*.json` file whose name starts with the
> current UTC date.

Given a `resolve_output_path` unit call with `None` and a fixed
timestamp:

> When `resolve_output_path(None, T)` is called,
> then it returns a path whose components are `.rprof/<T>.json`.

## Notes

- The decision to write under `./.rprof/` rather than `/tmp/` is so the
  reports stay alongside the code they profile. CI artifacts can pick
  them up with a glob on `.rprof/*.json`.
- A future enhancement (`output-atomic-write`) could borrow Bear's
  temp-file-plus-rename pattern. Deferred until corruption is observed
  in practice.
