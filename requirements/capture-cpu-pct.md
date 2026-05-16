---
title: CPU percentage and CPU time semantics
status: implemented
---

## Intent

Two CPU numbers appear in every report and they mean different things:

- **Per-sample `cpu_user_pct` / `cpu_sys_pct`** — instantaneous "how busy
  is the child right now?", computed from tick deltas between adjacent
  samples. This is what the charts plot.
- **Summary `user_cpu_ms` / `system_cpu_ms`** — total CPU time the child
  consumed over the whole run, as the kernel saw it. This is what shows
  up in the summary table and is the basis for cross-run comparisons.

Users need both to be unambiguous and consistent. Surprising values
(e.g. summary CPU lower than the per-sample integral, per-sample CPU
above 100 % on a uniprocessor) need clear documented reasons.

## Acceptance criteria

- The first sample of every run has `cpu_user_pct = 0` and
  `cpu_sys_pct = 0`. There is no previous sample to delta against, and
  rprof never invents a starting tick value.
- For subsequent samples, the CPU percentage is computed as
  `(delta_ticks / sysconf(_SC_CLK_TCK)) / (delta_t_seconds) * 100`,
  with `delta_t` being the monotonic wall time between adjacent
  samples.
- 100 % means one fully-loaded core. A process pegging four cores
  reads as ~400 %; the value is *not* normalised against the host's
  total CPU count. Users compare against the count in `host.cpu_count`
  if they want a fraction-of-machine view.
- `summary.user_cpu_ms` and `summary.system_cpu_ms` come from
  `getrusage(RUSAGE_CHILDREN)` after `wait()` returns. They are
  cumulative across all waited-for children of `rprof` (which is one
  child for v1).
- The summary values may differ from the trapezoidal integral of the
  per-sample percentages, because:
  - Short-lived grandchildren that lived between samples are not
    visible in the per-sample track but *are* included in the
    `getrusage` total (when `--include-children` is on or the kernel
    accounts them under the direct child).
  - `getrusage` is accurate to microseconds; the per-sample track is
    only as fine-grained as `--interval`.

## Non-functional constraints

- The computation must use the elapsed monotonic time between the two
  sample timestamps, never the requested sample interval. If the OS
  scheduler delays the polling thread, the reported interval would be
  wrong, but the actual time delta still produces correct CPU %.
- The CPU values must be valid `f64`s, not `NaN` or `inf`. The
  `delta_t_seconds = max(actual, 1ms)` floor guards against the
  degenerate case where two samples land in the same millisecond.

## Implementation details

- `compute_samples()` in `src/runner.rs` does the per-sample
  derivation. It runs in a single pass over `(t_ms, wall_ms,
  RawSample)` tuples produced by the sampler thread.
- `read_rusage_children()` in `src/runner.rs` calls
  `libc::getrusage(RUSAGE_CHILDREN, ...)` after the child has been
  reaped and converts the two `timeval` fields to milliseconds.
- `clock_ticks_per_second()` returns `sysconf(_SC_CLK_TCK)` (default
  100 on Linux), defaulting to 100.0 on the unlikely failure path.

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

> When `compute_samples()` is given a single raw sample,
> then the resulting `Sample` has `cpu_user_pct == 0.0` and
> `cpu_sys_pct == 0.0`.

Given two samples one second apart, with `CLK_TCK` user ticks burned
between them:

> When `compute_samples()` processes the pair,
> then the second sample's `cpu_user_pct` is approximately 100.0
> (within 0.5 of the target).

Given two samples 500 ms apart, with `CLK_TCK` user ticks burned:

> When `compute_samples()` processes the pair,
> then the second sample's `cpu_user_pct` is approximately 200.0
> (within 1.0 of the target). This pins the "100% = one core"
> convention.

Given a CPU-busy workload of at least 250 ms:

> When `rprof run -- <busy>` runs,
> then `summary.user_cpu_ms + summary.system_cpu_ms > 0`.

## Notes

- The decision to report per-core percentages (not fraction-of-machine)
  matches what every other Unix profiler does and avoids confusion
  when the user already knows their core count.
- The two CPU numbers in the schema (per-sample % and summary ms)
  intentionally describe different things; users should not expect
  them to numerically reconcile through trapezoidal integration.
- Related: [`capture-proc-backend`](capture-proc-backend.md),
  [`schema-v1`](schema-v1.md).
