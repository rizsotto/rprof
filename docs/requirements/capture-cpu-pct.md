---
title: CPU percentage and CPU time semantics
status: implemented
---

## Intent

Two CPU numbers appear in every report and they mean different things:

- **Per-sample CPU %** — instantaneous "how busy is the child right
  now?", derived by the *reader* from the cumulative `utime_ticks`
  and `stime_ticks` carried on each sample. This is what the charts
  plot.
- **Footer `user_cpu_ms` / `system_cpu_ms`** — total CPU time the
  child consumed over the whole run, as the kernel saw it via
  `getrusage`. This is what shows up in the summary table and is the
  basis for cross-run comparisons.

Users need both to be unambiguous and consistent. Surprising values
(e.g. summary CPU lower than the per-sample integral, per-sample CPU
above 100 % on a uniprocessor) need clear documented reasons.

## Acceptance criteria

- Per-sample CPU is **not** stored on disk; the schema records
  cumulative `utime_ticks` / `stime_ticks` and the reader computes
  percentages. The first sample of every run reads as
  `cpu_user_pct = 0` and `cpu_sys_pct = 0`: there is no previous
  sample to delta against, and rprof never invents a starting tick
  value.
- For subsequent samples, the reader computes
  `(delta_ticks / clock_ticks_per_sec) / (delta_t_seconds) * 100`,
  with `delta_t` being the elapsed monotonic time between adjacent
  samples and `clock_ticks_per_sec` carried on the report's header.
- 100 % means one fully-loaded core. A process pegging four cores
  reads as ~400 %; the value is *not* normalised against the host's
  total CPU count. Users compare against the count in `host.cpu_count`
  if they want a fraction-of-machine view.
- `footer.user_cpu_ms` and `footer.system_cpu_ms` come from
  `getrusage(RUSAGE_CHILDREN)` after `wait()` returns. They are
  cumulative across all waited-for children of `rprof` (which is one
  child for v1).
- The footer values may differ from the trapezoidal integral of the
  per-sample percentages, because `getrusage` is accurate to
  microseconds while the per-sample track is only as fine-grained as
  `--interval`. Bursts shorter than the interval show up in the
  footer but not on the chart.

## Non-functional constraints

- The computation must use the elapsed monotonic time between the two
  sample timestamps, never the requested sample interval. If the OS
  scheduler delays the polling thread, the reported interval would be
  wrong, but the actual time delta still produces correct CPU %.
- The CPU values must be valid `f64`s, not `NaN` or `inf`. The
  `delta_t_seconds = max(actual, 1ms)` floor guards against the
  degenerate case where two samples land in the same millisecond.

## Known limitations

- A child that uses more than one core may produce CPU% greater than
  100 %. This is intentional and matches the convention of `top` and
  `htop` in their default "per-core %" mode.
- The first sample always reads 0 % CPU even when the child started
  busy. This is a polling-method artefact, not a bug; the second
  sample onwards is correct.
- The `getrusage(RUSAGE_CHILDREN)` total includes any child that
  `rprof` waited for, not just the specific one being profiled.
  `rprof` only spawns one child, so this is currently moot, but
  callers that wrap `rprof` in shell pipelines should not double-spawn
  children through it.

## Testing

Given a single-sample run:

> When the viewer derives per-sample CPU%,
> then the single sample has `cpu_user_pct == 0.0` and
> `cpu_sys_pct == 0.0`.

Given two samples one second apart, with `CLK_TCK` user ticks burned
between them:

> When the viewer derives per-sample CPU% for the pair,
> then the second sample's `cpu_user_pct` is approximately 100.0
> (within 0.5 of the target).

Given a CPU-busy workload of at least 250 ms:

> When `rprof run -- <busy>` runs,
> then `footer.user_cpu_ms + footer.system_cpu_ms > 0` in the
> resulting report.

## Notes

- The decision to report per-core percentages (not fraction-of-machine)
  matches what every other Unix profiler does and avoids confusion
  when the user already knows their core count.
- The two CPU numbers in the schema (per-sample % and summary ms)
  intentionally describe different things; users should not expect
  them to numerically reconcile through trapezoidal integration.
- Related: [`capture-proc-backend`](capture-proc-backend.md),
  [`schema-v1`](schema-v1.md).
