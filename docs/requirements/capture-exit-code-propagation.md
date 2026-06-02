---
title: Exit code propagation
status: implemented
---

## Intent

Shells, CI runners, and parent `make` rules read `rprof`'s exit code to
decide whether the profiled command succeeded. `rprof` must not invent an
exit code: the caller sees the real result of the workload, not a code
shaped by the profiler.

## Acceptance criteria

- When the child exits normally with code `N`, `rprof` exits with code
  `N` truncated to the low 8 bits (`N & 0xff`), matching what the shell
  sees in `$?`.
- When the child is terminated by signal `S`, `rprof` exits with
  `128 + S` (clamped to 255).
- When the child cannot be spawned (for example, the program does not
  exist), `rprof` exits with a non-zero status and prints an error to
  stderr.

## Non-functional constraints

- The mirroring must hold for every supported child outcome, not just the
  common `0` / non-zero distinction. CI systems that look for specific
  exit codes (such as Bash's `127` for command-not-found) rely on it.

## Known limitations

- The high bits of a child's exit code (anything above 255) are
  unrecoverable from the shell, so `rprof` truncates rather than mapping
  to 1. This matches Bash, dash, and zsh behaviour.
- A child that exits with code 0 *but is then signalled* (a rare
  race) is reported as exit 0; whichever event the kernel reports first
  wins.

## Testing

Given a successful child:

> When the user runs `rprof run -- true`,
> then `rprof` exits with code 0.

Given a child that exits non-zero:

> When the user runs `rprof run -- sh -c "exit 42"`,
> then `rprof` exits with code 42.

Given the `exit_status_to_u8` mapping in isolation:

> When `code = Some(258)`, the function returns 2 (truncation).
> When `code = None, signal = Some(2)` (SIGINT), the function returns 130.
> When `code = None, signal = Some(15)` (SIGTERM), the function returns 143.

## Notes

- The shell convention of `128 + signum` is not always observable when
  the child handled the signal itself, because the kernel reports the
  child as having exited with the handler's chosen code. That is correct
  behaviour: rprof reports what actually happened.
- Related: `capture-signal-forwarding`.
