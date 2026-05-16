---
title: Process tree aggregation
status: implemented
---

## Intent

By default, `rprof run` measures only the direct child it spawned. This
matches the user's mental model of "I asked rprof to run this one command,
and the numbers describe that command." For build scripts and other
workloads where the interesting work happens in grandchildren (a wrapper
shell that exec's `cargo`, which spawns `rustc`, which spawns `ld`),
`--include-children` aggregates metrics across the whole process tree
rooted at the direct child.

## Acceptance criteria

- Without `--include-children`, every sample contains only the direct
  child's `/proc/<pid>` data.
- With `--include-children`, every sample sums RSS, VSZ, CPU ticks,
  thread count, open FDs, and IO bytes across the direct child plus
  every transitive descendant alive at sample time.
- The choice is recorded in the report: `run.include_children` is
  `true` or `false` matching the flag.
- Descendants that die between samples are simply absent from later
  samples; they are not double-counted.
- Descendants that appear after sample N start contributing at sample
  N+1.

## Non-functional constraints

- The tree walk must tolerate processes disappearing mid-walk. A
  vanished PID between `readdir` and `read_to_string` of `children` is
  not an error.
- The walk has a finite per-tick cost. The default 100 ms interval is
  the budget; if the tree grows to hundreds of processes, the sampler
  should still complete within that budget on commodity hardware.

## Implementation details

- The sampler maintains a stack of pending PIDs and a seen-set to
  prevent revisits. For each PID it reads `/proc/<pid>/task/*/children`
  to enumerate children, then samples each child via the same
  `sample_pid()` path.
- Aggregation uses saturating arithmetic so a pathological tree cannot
  overflow `u64`.
- The walk is purely from the root child downward; rprof never samples
  itself or unrelated processes.

## Known limitations

- Short-lived grandchildren that live for less than the sample interval
  are missed entirely. This is fundamental to the polling approach.
  Documented in `idea.md`'s "Risks and open questions".
- VSZ is summed naively. Two processes that share memory mappings will
  appear to use twice as much VSZ as they really do. This matches what
  most `ps`-style tools report.
- Open FD counts are summed too. A process and its `fork()`ed child
  share open FDs but the count adds them, which can over-report.
- The reported sample is a single snapshot of cumulative counters;
  CPU% is derived from deltas at the runner level
  (`compute_samples()` in `src/runner.rs`).

## Testing

Given a process with no children:

> When the sampler with `include_children = true` polls it,
> then the returned sample is well-formed (no I/O errors)
> and equals what the single-process sampler would return for the same
> tick.

Given a script that spawns N children that live for several seconds:

> When `rprof run --include-children -- script` runs,
> then at least one sample reports a higher thread count and RSS than
> the direct-child-only run would.

## Notes

- For accurate tree accounting that catches short-lived grandchildren,
  the cgroup-v2 backend (deferred to phase 4) is the correct fix.
- Related: `capture-proc-backend`.
