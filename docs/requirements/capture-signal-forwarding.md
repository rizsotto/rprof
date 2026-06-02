---
title: Signal forwarding and report-on-exit guarantee
status: implemented
---

## Intent

When a user presses Ctrl-C, or a CI runner sends SIGTERM, the signal must
reach the child being profiled — not be swallowed by `rprof`. And the
report must still land on disk before `rprof` exits, because in CI the
most valuable runs are precisely the ones that died early and need
post-mortem analysis. Dropping the report on Ctrl-C would be a real
foot-gun.

## Acceptance criteria

- SIGINT, SIGTERM, and SIGHUP delivered to `rprof` are forwarded to the
  child process.
- After the child terminates (whether by signal or normal exit), `rprof`
  writes a `footer` record to the report and flushes the file before
  exiting. The header and the sample stream are already on disk by the
  time the signal arrives (see
  [`capture-streaming-write`](capture-streaming-write.md)), so the
  resulting file is a complete report.
- The footer records the cause of termination: `exit_code` is set when
  the child returned a code, or `signal` is set when the child was
  killed by a signal. They are mutually exclusive in practice (the
  kernel reports one or the other), but `null` is allowed for both
  during early failures before the child started.
- The latency between the signal arriving and `rprof` exiting is bounded
  by the sample interval (default 100 ms).
- The exit-code mirroring contract itself is owned by
  [`capture-exit-code-propagation`](capture-exit-code-propagation.md);
  this requirement only guarantees that the report is on disk by the
  time the mirroring happens.

## Non-functional constraints

- The signal handler must be async-signal-safe: only an atomic load and
  `libc::kill` are allowed inside it. No allocation, no `printf`, no lock
  acquisition.

## Known limitations

- SIGKILL cannot be caught. If `rprof` itself is SIGKILL'd, no report is
  written. The acceptance only covers signals that the process can catch.
- The specific signal that terminated `rprof` is not encoded in `rprof`'s
  exit code beyond the `128 + signum` convention. Callers cannot
  distinguish SIGINT from SIGTERM by exit code alone.
- Daemon-style grandchildren that detach from the child may keep running
  after `rprof` exits. This is fundamental to the polling model.

## Testing

Given a long-running child under `rprof`:

> When the test sends SIGINT to `rprof` while the child is alive,
> then both `rprof` and the child terminate within a few hundred
> milliseconds,
> and the report exists on disk with a footer record,
> and the footer's `signal` or `exit_code` field reflects how the
> child died.

Given a child that exits normally:

> When the user runs `rprof run -- true`,
> then `rprof` exits with status 0,
> and the report's footer has `exit_code = 0` and `signal = null`.

Given a child that exits non-zero:

> When the user runs `rprof run -- sh -c "exit 42"`,
> then `rprof` exits with status 42,
> and the report's footer has `exit_code = 42`.

## Notes

- Related: `capture-exit-code-propagation`. That requirement covers the
  exit-code mirroring contract; this one covers the signal-handling
  mechanics and the report-persistence guarantee.
- The 100 ms upper bound on signal-to-exit latency comes from the fact
  that the sampler thread sleeps on `recv_timeout(interval)`; aggressive
  use cases that need sub-100 ms shutdown should lower `--interval`.
