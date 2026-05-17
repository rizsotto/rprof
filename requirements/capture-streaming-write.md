---
title: Streaming sample writes (no in-memory buffer)
status: proposed
---

## Intent

A profiler that buffers every sample in memory until the run ends is
fine for short jobs but wrong for the use cases that matter: hours-long
test suites, batch builds, anything that might OOM or get SIGKILL'd
before it can exit normally. `rprof` must write samples to disk as they
are captured, not at the end, so that:

- A run of any length uses bounded memory: a small write buffer rather
  than one record per sample.
- A `kill -9` of `rprof`, an OOM kill, or a host power loss leaves a
  usable partial file (the header plus whatever samples reached disk),
  not nothing.
- Operators can `tail -f` the output file of a long-running capture
  and watch the metric stream as it grows.

## Acceptance criteria

- `rprof run` emits records in capture order:
  1. A header record is emitted immediately after the child is
     spawned, before the first sample is taken.
  2. A sample record is emitted within the same sampling tick in which
     the sample was captured.
  3. A footer record is emitted after the child has exited, before
     `rprof` itself exits.
- The capture path never retains more than a constant number of
  in-flight sample records. That bound is set by the underlying write
  buffer and is independent of the run's duration or sample count.
- Each sample record reaches the kernel before the next sampling tick
  begins. Durable on-disk persistence (`fsync`) is **not** required.
- If `rprof` is killed by an uncatchable signal (SIGKILL) or the host
  loses power mid-run, the resulting file contains the header record
  plus every sample record that reached the kernel before the kill,
  and no footer record. The viewer treats such a file as a partial
  report (per [`schema-v1`](schema-v1.md)).
- The capture path makes **no** second pass over the captured samples.
  Anything that needs cross-sample state (e.g. computing CPU% from
  cumulative ticks, or finding `peak_rss`) is the reader's job and
  runs at view-time, not at capture-time.

## Non-functional constraints

- Per-sample write overhead must stay well below the sample interval
  at the default `--interval 100ms`. Flushing the userspace buffer
  per tick is acceptable; `fsync` per tick is not.
- Steady-state resident-set growth of `rprof` itself during a long
  capture must be flat (within measurement noise). A run of N minutes
  and a run of 10×N minutes should occupy the same RSS at steady
  state.

## Known limitations

- Atomic-rename semantics (write to a temp file, then rename) are
  incompatible with streaming and are not used. A concurrent reader
  may see a partial last record; the schema requires the reader to
  tolerate that.
- Durable on-disk persistence is not guaranteed. A host that loses
  power between the write and the disk flush may still lose the most
  recent samples. This is a deliberate trade — per-tick fsync would
  dominate the per-tick cost on rotational media without meaningfully
  improving the typical failure mode (process kill, not power loss).
- The output file implicitly assumes a single writer. A second
  `rprof` process opening the same path concurrently is undefined.
- Backfill or re-ordering of samples is not supported. Records are
  emitted in capture order; if a tick is slow, the next tick's
  `t_ms` reflects real elapsed time, but there is no compensation
  pass.

## Testing

Given a long-running child under `rprof`:

> When the test reads the output file once per second while the run
> is in progress,
> then the count of sample records visible grows monotonically with
> wall-clock time.

Given a `rprof` process killed mid-run with SIGKILL:

> When the test reads the resulting `.jsonl` file after the kill,
> then the file contains a header record,
> and at least one sample record,
> and no footer record.

Given a normal short run:

> When `rprof run -- sleep 0.3` completes,
> then the output file contains exactly one header record,
> followed by one or more sample records in ascending `t_ms` order,
> followed by exactly one footer record.

Given a long synthetic capture (e.g. ten minutes at 100ms interval):

> When `rprof`'s RSS is sampled at the start and at the end of the
> capture,
> then the end-of-run RSS is within a small constant of the
> start-of-run RSS (no growth proportional to sample count).

## Notes

- Related: [`schema-v1`](schema-v1.md) (the on-disk format this
  behaviour produces),
  [`capture-signal-forwarding`](capture-signal-forwarding.md) (the
  graceful-shutdown contract this behaviour interacts with), and
  [`capture-output-path`](capture-output-path.md) (the file the
  records are written to).
