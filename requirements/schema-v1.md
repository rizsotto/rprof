---
title: JSON report schema (v1)
status: implemented
---

## Intent

The on-disk JSON report is rprof's only durable interface between the
capture step and everything that consumes it: the viewer, archives, diff
tooling, ad-hoc `jq` analysis. Its shape must be predictable, versioned,
and forward-compatible enough that adding fields does not break old
viewers.

This requirement is the authoritative specification of the JSON shape.
A reimplementation in any language must produce reports that match the
field names, types, and semantics described here.

## Acceptance criteria

- Every report has a top-level integer `schema_version` field. v1 is `1`.
- All field names are `snake_case`.
- The top-level object has these keys, in this order:
  `schema_version`, `tool`, `run`, `host`, `summary`, `samples`.
- Adding new optional fields to any object (`run`, `sample`, `summary`,
  `host`) is forward-compatible: the writer keeps producing
  `schema_version = 1`. Bumping the major is reserved for a *breaking*
  change. Readers MUST tolerate (ignore) unknown fields.
- The viewer in `rprof view` refuses to render reports whose
  `schema_version` does not equal its own. The error message names the
  file and the unexpected version.

### Field reference

In the type column: *non-negative integer* maps to JSON number (no
fractional part, ≥ 0); *signed integer* maps to JSON number (no
fractional part, may be negative); *number* maps to JSON number
(fractional allowed); *string* maps to JSON string; *X or null* maps to
the JSON value of type X, or JSON `null`.

#### Top-level

| Field            | Type                  | Notes |
|------------------|-----------------------|-------|
| `schema_version` | non-negative integer  | `1` for this version. |
| `tool`           | object (see below)    | Identifies the writer. |
| `run`            | object (see below)    | What was captured. |
| `host`           | object (see below)    | Where it was captured. |
| `summary`        | object (see below)    | Whole-run aggregates. |
| `samples`        | array of sample objects | One entry per polling tick, in ascending `t_ms` order. |

#### `tool`

| Field     | Type   | Notes |
|-----------|--------|-------|
| `name`    | string | Always `"rprof"` for reports rprof writes. |
| `version` | string | Semver string of the writer. |

#### `run`

| Field                  | Type                       | Notes |
|------------------------|----------------------------|-------|
| `command`              | array of strings           | Program plus arguments, as forwarded after `--`. Non-empty. |
| `cwd`                  | string                     | Working directory at capture time. |
| `env_fingerprint`      | string                     | SHA-256 hex digest of the sorted `KEY=VALUE\n`-joined environment. 64 lowercase hex chars. The full environment is **not** stored, so the report cannot leak secrets. |
| `start_time`           | string                     | RFC 3339 / ISO 8601 with millisecond precision in UTC (e.g. `"2026-05-14T10:30:00.000Z"`). |
| `wall_duration_ms`     | non-negative integer       | Wall-clock duration of the run, in milliseconds. |
| `exit_code`            | signed integer or null     | The child's exit code, or `null` if it was killed by a signal or never started. |
| `signal`               | signed integer or null     | The signal that killed the child, or `null` if it exited normally or never started. At most one of `exit_code` / `signal` is non-null. |
| `backend`              | string                     | Identifier of the sampling backend (e.g. `"proc"` for Linux `/proc` polling). |
| `sample_interval_ms`   | non-negative integer       | The requested poll interval, in milliseconds. |

#### `host`

| Field                | Type                  | Notes |
|----------------------|-----------------------|-------|
| `hostname`           | string                | Host the capture ran on. |
| `kernel`             | string                | e.g. `"Linux 6.8.0"`. |
| `cpu_count`          | non-negative integer  | Logical CPUs visible to the process. |
| `total_memory_bytes` | non-negative integer  | Total physical memory in bytes. |

#### `summary`

| Field             | Type                  | Notes |
|-------------------|-----------------------|-------|
| `peak_rss_bytes`  | non-negative integer  | Maximum `rss_bytes` across all samples. |
| `user_cpu_ms`     | non-negative integer  | Total user CPU time the child consumed (from `getrusage(RUSAGE_CHILDREN)`), in milliseconds. May differ from the integral of `cpu_user_pct` because rusage is microsecond-accurate while the sample track is `--interval`-grained. |
| `system_cpu_ms`   | non-negative integer  | Total system CPU time, same source. |
| `sample_count`    | non-negative integer  | Length of `samples`. |

#### `sample` (one element of `samples`)

| Field             | Type                  | Notes |
|-------------------|-----------------------|-------|
| `t_ms`            | non-negative integer  | Monotonic offset from run start, in milliseconds. The first sample is at `0`. |
| `wall_ms`         | non-negative integer  | Absolute Unix epoch time in milliseconds at the moment of the sample. Two runs can be aligned on this for time-axis diffs. |
| `cpu_user_pct`    | number                | Instantaneous user CPU usage, per-core (100 % = one pegged core; a 4-core-pegged process reads as ~400). First sample is `0`. |
| `cpu_sys_pct`     | number                | Instantaneous system CPU usage, same convention. First sample is `0`. |
| `rss_bytes`       | non-negative integer  | Resident set size in bytes (pages × page size). |
| `vsz_bytes`       | non-negative integer  | Virtual size in bytes. |
| `threads`         | non-negative integer  | Thread count. |
| `open_fds`        | non-negative integer  | Number of open file descriptors. |
| `io_read_bytes`   | non-negative integer  | Cumulative bytes read since process start. May stay `0` if the backend cannot read IO counters. |
| `io_write_bytes`  | non-negative integer  | Cumulative bytes written since process start. |

### Canonical example

A minimal but complete report with one sample:

```json
{
  "schema_version": 1,
  "tool": {"name": "rprof", "version": "0.1.0"},
  "run": {
    "command": ["sleep", "0.1"],
    "cwd": "/tmp",
    "env_fingerprint": "0000000000000000000000000000000000000000000000000000000000000000",
    "start_time": "2026-05-14T10:30:00.000Z",
    "wall_duration_ms": 100,
    "exit_code": 0,
    "signal": null,
    "backend": "proc",
    "sample_interval_ms": 100
  },
  "host": {
    "hostname": "h",
    "kernel": "Linux 6.8.0",
    "cpu_count": 4,
    "total_memory_bytes": 17179869184
  },
  "summary": {
    "peak_rss_bytes": 1048576,
    "user_cpu_ms": 12,
    "system_cpu_ms": 3,
    "sample_count": 1
  },
  "samples": [
    {
      "t_ms": 0,
      "wall_ms": 1700000000000,
      "cpu_user_pct": 0.0,
      "cpu_sys_pct": 0.0,
      "rss_bytes": 1048576,
      "vsz_bytes": 2097152,
      "threads": 1,
      "open_fds": 4,
      "io_read_bytes": 0,
      "io_write_bytes": 0
    }
  ]
}
```

This exact document is also embedded as a string constant in
`src/schema.rs` and parsed by the unit test
`canonical_example_parses` — so the example here cannot silently rot
out of sync with the writer.

## Non-functional constraints

- The per-sample object layout is intentionally row-major (one JSON
  object per timestamp) rather than columnar (parallel arrays per
  metric). The rationale: low memory use during capture, simple
  streaming, and trivial `jq` access. Columnar is ~3x smaller on disk
  but adds complexity; row-major was chosen at v1 design time.
- Unknown fields are tolerated on read. The unit test
  `additive_fields_tolerated_on_read` pins this behaviour against
  future drift.

## Implementation details

- The writer's struct layout lives in `src/schema.rs`. Field order in
  the emitted JSON matches the struct field order (serde default), so
  the order specified in the Field reference above is also what shows
  up on disk.
- `SCHEMA_VERSION` is a constant (`1`). The viewer compares against
  this when loading a report.

## Known limitations

- v1 is frozen. Future schema changes are either additive (no version
  bump) or require shipping v2 with a viewer that handles both.
- The host metadata is captured on the machine that ran the capture,
  not the machine that views the report. This is intentional.

## Testing

Given a round-trip through a JSON serializer:

> When a report is serialised and deserialised,
> then the deserialised report equals the original.

Given the canonical example document above:

> When a reader parses it,
> then the parse succeeds and every documented field is reachable at
> its specified path with its specified type.

Given a JSON report with an extra unknown field at the `run` level:

> When `rprof view` reads it,
> then the read succeeds and the unknown field is ignored.

Given a JSON report with `schema_version = 999`:

> When `rprof view` is asked to render it,
> then the viewer exits with a non-zero status and the error message
> mentions `schema_version`.

## Notes

- Related: `viewer-self-contained-html` (which consumes this schema).
