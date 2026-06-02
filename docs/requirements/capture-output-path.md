---
title: Report output path resolution
status: implemented
---

## Intent

Most users want to control where the JSONL report lands; for ad-hoc runs they
just want it to land *somewhere* without picking a name. Passing `-o` is
the explicit form; omitting it auto-generates a path under `./.rprof/`
keyed by start time so successive runs do not clobber each other.

## Acceptance criteria

- With `-o <path>`, `rprof` writes the report to exactly `<path>`. If
  the parent directory does not exist, `rprof` creates it.
- Without `-o`, `rprof` writes the report to
  `./.rprof/<YYYY-MM-DDTHHMMSS>.jsonl` (UTC) in the working directory.
  `rprof` creates the `.rprof/` directory if needed.
- If `-o`'s parent directory cannot be created (permissions), `rprof`
  fails with a clear error after the child has exited.
- The report is written **as a stream**: the header lands on disk
  immediately after the child is spawned, each sample is appended as
  it is captured, and the footer is appended once the child has
  exited. Atomic-rename semantics are deliberately not used; see
  [`capture-streaming-write`](capture-streaming-write.md) for the
  rationale and the partial-file guarantees a reader can rely on.

## Non-functional constraints

- The auto-generated filename is sortable lexicographically by time so
  shell globbing in chronological order works (`.rprof/*.jsonl | sort`).
- The timestamp format is filesystem-safe on Linux and macOS (no colons,
  no slashes). Windows compatibility is not required for v1.

## Known limitations

- Atomic-rename semantics (write to `.tmp`, then `rename(2)`) are not
  implemented and are fundamentally incompatible with streaming
  writes. A reader that opens the file concurrently with the writer
  may see a partial last record; the on-disk schema requires readers
  to tolerate this.
- The fallback path is always relative to `rprof`'s current working
  directory. There is no `RPROF_OUTPUT_DIR` env var override yet.

## Testing

Given an explicit output path:

> When the user runs `rprof run -o /tmp/out/r.jsonl -- true`,
> then `/tmp/out/` is created if absent,
> and `/tmp/out/r.jsonl` contains the JSONL report.

Given no output flag:

> When the user runs `rprof run -- true` inside an empty directory,
> then a `.rprof/` subdirectory is created,
> and it contains exactly one `*.jsonl` file whose name starts with the
> current UTC date.

Given a `resolve_output_path` unit call with `None` and a fixed
timestamp:

> When `resolve_output_path(None, T)` is called,
> then it returns a path whose components are `.rprof/<T>.jsonl`.

## Notes

- The decision to write under `./.rprof/` rather than `/tmp/` is so the
  reports stay alongside the code they profile. CI artifacts can pick
  them up with a glob on `.rprof/*.jsonl`.
- Related: [`capture-streaming-write`](capture-streaming-write.md) and
  [`schema-v1`](schema-v1.md) (the on-disk shape the path holds).
