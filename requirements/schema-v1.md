---
title: JSONL report schema
status: proposed
---

## Intent

The on-disk report is rprof's only durable interface between the capture
step and everything that consumes it: the viewer, archives, diff tooling,
ad-hoc `jq` analysis. Its shape must be predictable, versioned, and
forward-compatible enough that adding fields does not break old viewers.

The format is **JSON Lines** (one JSON object per line). This lets the
runner append records to disk as they are captured, so a long run does
not have to be held in memory and a `kill -9` of rprof (or the host)
leaves a usable partial file rather than nothing. See
[`capture-streaming-write`](capture-streaming-write.md) for the capture
behaviour this format enables.

This requirement is the authoritative specification of the on-disk
shape. A reimplementation in any language must produce reports that
match the record types, field names, types, and semantics described
here.

## Acceptance criteria

- A report is a UTF-8 text file containing one JSON object per line,
  separated by `\n` (LF). The file extension is `.jsonl`.
- Each record is a JSON object with a `type` field that names the
  record kind. Three kinds are defined: `header`, `sample`, `footer`.
- A well-formed report begins with exactly one `header` line, followed
  by zero or more `sample` lines in ascending `t_ms` order, optionally
  followed by exactly one `footer` line.
- The `header` carries a `schema` integer naming the schema revision.
  For this revision the value is `1`.
- Readers MUST refuse files whose `header.schema` does not match the
  revision they understand. The error message names the file and the
  unexpected revision.
- Readers MUST tolerate (ignore) unknown fields on any record, and
  MUST skip lines whose `type` value they do not recognise.
- Readers MUST tolerate a missing or truncated final line. If the run
  was interrupted, the file may end mid-record; the viewer treats such
  a file as a partial report and renders the samples that did parse.
- Adding new optional fields to any existing record is
  forward-compatible and does not bump `schema`. Defining a new
  record type does not bump it either, as long as old readers can
  skip the unknown lines.

### Field reference

Type conventions: *non-negative integer* (JSON number, no fractional
part, ≥ 0); *signed integer* (JSON number, no fractional part, may be
negative); *number* (JSON number, fractional allowed); *string* (JSON
string); *X or null* (a value of type X, or JSON `null`).

#### `header` (first line)

| Field    | Type                  | Notes |
|----------|-----------------------|-------|
| `type`   | string                | Always `"header"`. |
| `schema` | non-negative integer  | `1` for this revision. |
| `tool`   | object                | Identifies the writer. See below. |
| `run`    | object                | What is being captured. See below. |
| `host`   | object                | Where it was captured. See below. |

The `header` is written immediately after the child is spawned, before
any samples. Fields that are only known after the child exits
(`wall_duration_ms`, `exit_code`, `signal`, `user_cpu_ms`,
`system_cpu_ms`) live on the `footer`, not the header.

##### `tool`

| Field     | Type   | Notes |
|-----------|--------|-------|
| `name`    | string | Always `"rprof"` for reports rprof writes. |
| `version` | string | Semver string of the writer. |

##### `run` (on header)

| Field                | Type                  | Notes |
|----------------------|-----------------------|-------|
| `command`            | array of strings      | Program plus arguments, as forwarded after `--`. Non-empty. |
| `cwd`                | string                | Working directory at capture time. |
| `env_fingerprint`    | string                | SHA-256 hex digest of the sorted `KEY=VALUE\n`-joined environment. 64 lowercase hex chars. The full environment is **not** stored, so the report cannot leak secrets. |
| `start_time`         | string                | RFC 3339 / ISO 8601 with millisecond precision in UTC (e.g. `"2026-05-14T10:30:00.000Z"`). |
| `backend`            | string                | Identifier of the sampling backend (e.g. `"proc"` for Linux `/proc` polling). |
| `sample_interval_ms` | non-negative integer  | The requested poll interval, in milliseconds. |

##### `host`

| Field                  | Type                  | Notes |
|------------------------|-----------------------|-------|
| `hostname`             | string                | Host the capture ran on. |
| `kernel`               | string                | e.g. `"Linux 6.8.0"`. |
| `cpu_count`            | non-negative integer  | Logical CPUs visible to the process. |
| `total_memory_bytes`   | non-negative integer  | Total physical memory in bytes. |
| `clock_ticks_per_sec`  | non-negative integer  | Platform `sysconf(_SC_CLK_TCK)` value. Readers use this to convert per-sample `utime_ticks`/`stime_ticks` to seconds. Carried once here rather than on every sample. |

#### `sample` (zero or more lines)

| Field            | Type                  | Notes |
|------------------|-----------------------|-------|
| `type`           | string                | Always `"sample"`. |
| `t_ms`           | non-negative integer  | Monotonic offset from run start, in milliseconds. The first sample is at `0`. |
| `wall_ms`        | non-negative integer  | Absolute Unix epoch time in milliseconds at the moment of the sample. Two runs can be aligned on this for time-axis diffs. |
| `utime_ticks`    | non-negative integer  | Cumulative user-mode CPU ticks since process start, as reported by the backend. |
| `stime_ticks`    | non-negative integer  | Cumulative kernel-mode CPU ticks since process start. |
| `rss_bytes`      | non-negative integer  | Resident set size in bytes (pages × page size). |
| `vsz_bytes`      | non-negative integer  | Virtual size in bytes. |
| `threads`        | non-negative integer  | Thread count. |
| `open_fds`       | non-negative integer  | Number of open file descriptors. |
| `io_read_bytes`  | non-negative integer  | Cumulative bytes read since process start. May stay `0` if the backend cannot read IO counters. |
| `io_write_bytes` | non-negative integer  | Cumulative bytes written since process start. |

CPU is recorded as **cumulative ticks**, not instantaneous
percentage. Computing `cpu_user_pct` / `cpu_sys_pct` is the reader's
job: delta the ticks between consecutive samples and divide by
`clock_ticks_per_sec * dt_seconds`, multiplied by 100. This keeps each
line self-contained — no sample's content depends on a previous sample
— which is what makes a truncated file still parseable.

#### `footer` (last line, optional)

| Field              | Type                       | Notes |
|--------------------|----------------------------|-------|
| `type`             | string                     | Always `"footer"`. |
| `wall_duration_ms` | non-negative integer       | Wall-clock duration of the run, in milliseconds. |
| `exit_code`        | signed integer or null     | The child's exit code, or `null` if it was killed by a signal or never started. |
| `signal`           | signed integer or null     | The signal that killed the child, or `null` if it exited normally or never started. At most one of `exit_code` / `signal` is non-null. |
| `user_cpu_ms`      | non-negative integer       | Total user CPU time the child consumed (from `getrusage(RUSAGE_CHILDREN)`), in milliseconds. |
| `system_cpu_ms`    | non-negative integer       | Total system CPU time, same source. |

The footer carries the values that are only known after the child has
exited. A file without a footer is a partial report — the run was
interrupted before it could be written. The viewer treats this as
"unknown end state" rather than an error.

Whole-run aggregates such as `peak_rss_bytes` and `sample_count` are
**not** stored. Readers compute them from the samples on load.
Storing them would invite inconsistency in the truncation case.

### Canonical example

A minimal but complete report with one sample (three lines, one
trailing `\n`):

```
{"type":"header","schema":1,"tool":{"name":"rprof","version":"0.1.0"},"run":{"command":["sleep","0.1"],"cwd":"/tmp","env_fingerprint":"0000000000000000000000000000000000000000000000000000000000000000","start_time":"2026-05-14T10:30:00.000Z","backend":"proc","sample_interval_ms":100},"host":{"hostname":"h","kernel":"Linux 6.8.0","cpu_count":4,"total_memory_bytes":17179869184,"clock_ticks_per_sec":100}}
{"type":"sample","t_ms":0,"wall_ms":1700000000000,"utime_ticks":0,"stime_ticks":0,"rss_bytes":1048576,"vsz_bytes":2097152,"threads":1,"open_fds":4,"io_read_bytes":0,"io_write_bytes":0}
{"type":"footer","wall_duration_ms":100,"exit_code":0,"signal":null,"user_cpu_ms":12,"system_cpu_ms":3}
```

This exact document is also embedded as a string constant in
`src/schema.rs` and parsed by the unit test
`canonical_example_parses` — so the example here cannot silently rot
out of sync with the writer.

## Non-functional constraints

- Records are row-major (one JSON object per timestamp) rather than
  columnar (parallel arrays per metric). This is what makes
  append-as-you-go possible.
- Per-record byte size is small enough that one `write(2)` per sample
  is reasonable on common kernels; no record needs to be split across
  writes for atomicity.
- Each record line is single-line JSON (no embedded newlines). Pretty
  printing is the reader's job (`jq -C . file.jsonl`).
- Unknown fields and unknown record types are tolerated on read.

## Known limitations

- Only one schema revision is supported by any given build. There is
  no cross-revision migration path: a reader that does not match the
  file's `schema` value refuses the file outright.
- The host metadata is captured on the machine that ran the capture,
  not the machine that views the report. This is intentional.

## Testing

Given a header, sample, and footer round-trip through the writer:

> When the records are serialised line-by-line and re-read,
> then each record deserialises to a structure equal to the original.

Given the canonical example document above:

> When a reader parses it,
> then the parse succeeds and every documented field is reachable at
> its specified path with its specified type.

Given a file with an extra unknown field on the `header.run` object:

> When `rprof view` reads it,
> then the read succeeds and the unknown field is ignored.

Given a file whose final line is truncated mid-JSON (no closing brace,
no terminating `\n`):

> When `rprof view` reads it,
> then it renders the samples from the preceding well-formed lines
> and reports the run's end state as "unknown" (no footer parsed).

Given a header with `schema = 999`:

> When `rprof view` is asked to render the file,
> then the viewer exits with a non-zero status and the error message
> mentions both the file path and `schema`.

## Notes

- Related:
  [`capture-streaming-write`](capture-streaming-write.md) (the
  capture behaviour this format enables) and
  [`viewer-self-contained-html`](viewer-self-contained-html.md) (the
  consumer of this schema).
