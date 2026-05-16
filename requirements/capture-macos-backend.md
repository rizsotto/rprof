---
title: macOS capture backend (libproc)
status: planned
---

## Intent

`rprof run` is currently Linux-only because the sampler reads
`/proc/<pid>`. The project's "best-effort macOS" goal needs a backend
that fills `RawSample` from Apple's `libproc` /
`proc_pidinfo(PROC_PIDTASKINFO, ...)` family of calls. Once present, all
the rest of the pipeline (signal forwarding, schema, viewer, diff mode)
already works portably on macOS.

## Acceptance criteria (sketch, to firm up before implementation)

- A `macos` (or `libproc`) backend produces `RawSample` records on
  macOS without root privileges.
- The report's `run.backend` records the chosen identifier.
- Metrics that are not available on macOS (e.g. per-process IO bytes
  with the same semantics as `/proc/<pid>/io`) are filled with `0`
  and the discrepancy is documented in this requirement.
- The `Sampler` trait does not change shape; this is purely a new
  backend module wired in via `cfg(target_os = "macos")`.
- Existing integration tests that gate on `target_os = "linux"`
  either grow a macOS-equivalent or are explicitly marked as not
  applicable on macOS.

## Open questions

- **Process tree.** macOS does not expose `/proc/<pid>/task/*/children`;
  `proc_listchildpids` is the typical replacement but has different
  semantics around zombies and group membership. Document the
  difference and decide whether `--include-children` is supported on
  macOS at parity, with caveats, or not at all in the first cut.
- **IO accounting.** Per-process IO bytes are not directly readable;
  the equivalent of `proc_pid_rusage` provides cumulative IO but with
  coarser fields. Decide whether to map them to the existing
  `io_read_bytes` / `io_write_bytes` or leave them as `0` for the
  first cut.
- **Test host.** Integration tests need a macOS runner in CI; for v1
  CI is Linux-only.

## Notes

- Out of scope for v1 per [`../CLAUDE.md`](../CLAUDE.md)'s non-goals
  framing (Linux is primary; macOS is best-effort).
- Related: [`capture-proc-backend`](capture-proc-backend.md).
