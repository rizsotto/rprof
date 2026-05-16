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

## Acceptance criteria

- Every report has a top-level integer `schema_version` field. v1 is `1`.
- The schema has these top-level keys, in order:
  `schema_version`, `tool`, `run`, `host`, `summary`, `samples`.
- `samples` is an array of per-sample objects. Each sample carries
  `t_ms`, `wall_ms`, `cpu_user_pct`, `cpu_sys_pct`, `rss_bytes`,
  `vsz_bytes`, `threads`, `open_fds`, `io_read_bytes`, `io_write_bytes`.
- `t_ms` is a monotonic offset (in milliseconds) from the start of the
  run, measured with `Instant::now()`.
- `wall_ms` is the absolute Unix epoch time in milliseconds at the
  moment of the sample. Two runs can be aligned on this for time-axis
  diffs.
- `run.exit_code` and `run.signal` are nullable; at most one is non-null
  for a given run.
- Adding new optional fields to any object (run, sample, summary, host)
  is forward-compatible: the writer at v2-with-extra-field still
  produces `schema_version = 1`. Bumping the major is reserved for a
  *breaking* change.
- The viewer in `rprof view` refuses to render reports whose
  `schema_version` does not equal its own `SCHEMA_VERSION`. The error
  message names the file and the unexpected version.

## Non-functional constraints

- The per-sample object layout is intentionally row-major (one JSON
  object per timestamp) rather than columnar (parallel arrays per
  metric). The rationale: low memory use during capture, simple
  streaming, and trivial `jq` access. Columnar is ~3x smaller on disk
  but adds complexity; row-major was chosen at v1 design time.
- Unknown fields are tolerated on read (serde's default for derived
  structs). The unit test `additive_fields_tolerated_on_read` pins
  this behaviour against future drift.

## Implementation details

- All structs live in `src/schema.rs` with `#[derive(Serialize,
  Deserialize)]`.
- `SCHEMA_VERSION` is a `const u32 = 1`. The viewer checks
  `report.schema_version == SCHEMA_VERSION` and bails on mismatch.
- `start_time` is RFC 3339 with millisecond precision in UTC, so a
  human can read it without converting timestamps.
- `env_fingerprint` is the SHA-256 hex digest of the sorted
  `KEY=VALUE\n`-joined environment. The full environment is *not*
  stored, so the report cannot leak secrets.

## Known limitations

- v1 is frozen. Future schema changes are either additive (no version
  bump) or require shipping v2 with a viewer that handles both.
- The schema has no `notes`/`labels` field on the run itself, only via
  `--label` at view time. Adding annotations to a captured report
  is a future enhancement.
- The host metadata is captured on the machine that ran the capture,
  not the machine that views the report. This is intentional.

## Testing

Given a round-trip through serde_json:

> When a `Report` is serialised and deserialised,
> then the deserialised struct equals the original.

Given a JSON report with an extra unknown field at the `run` level:

> When `rprof view` reads it,
> then the read succeeds and the unknown field is ignored.

Given a JSON report with `schema_version = 999`:

> When `rprof view` is asked to render it,
> then the viewer exits with a non-zero status and the error message
> mentions `schema_version`.

## Notes

- `SCHEMA.md` (a human-readable cheat sheet of the schema) is planned
  but not yet written. The Rust struct definitions in
  `src/schema.rs` are the authoritative spec until then.
- Related: `viewer-self-contained-html` (which consumes this schema).
