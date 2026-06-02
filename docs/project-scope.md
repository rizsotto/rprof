# Project scope

rprof's value comes from staying small and predictable: one binary, one
process, one report. A resource profiler can grow in many directions —
stack sampling, distributed tracing, system-wide monitoring,
process-tree aggregation — and each pulls in a different acquisition
mechanism and a different mental model. This document fixes the scope in
writing so a feature request is weighed against a stated boundary rather
than re-litigated from scratch each time one arrives.

## Goals

- Single static binary with no runtime dependencies. Drop it into a
  container or CI image and it works.
- Capture is non-invasive: no instrumentation, no `LD_PRELOAD`, no
  `ptrace`. The target program is unaware it is being measured.
- Output is plain JSON Lines with a versioned schema. Trivially
  scriptable, diffable, archivable; `tail -f` works while a capture is
  in progress and `kill -9` leaves a partial-but-usable file rather
  than nothing.
- Visualisation is a self-contained HTML file. No long-running server,
  no port conflicts, no Python or Node required to view a report.
- Diffing multiple runs is a first-class feature, not an afterthought.

## Non-goals (v1)

These are deliberately out of scope; a requirement that touches one
needs a fresh decision, not a "while we're at it".

- Flamegraphs and stack sampling. These need a different acquisition
  path (`perf`, eBPF, DTrace) and a different visualisation; if ever
  built, they belong in a separate subcommand.
- Distributed tracing, network IO breakdown, GPU metrics, per-syscall
  accounting.
- Windows support. Linux is the primary target; macOS is best-effort.
- A long-running daemon or system-wide monitor. rprof measures one
  command invocation per process.
- Process-tree resource aggregation. rprof reports the usage of the
  single child it spawned. Summing across the tree double-counts shared
  memory and FDs, hides which descendant drove a spike, and conflicts
  with the "one process, one chart" mental model the viewer is built
  around. Users who genuinely need tree-wide accounting should reach
  for cgroup-level tools (`systemd-run --scope`, `cgexec`, `perf stat`);
  `/usr/bin/time -v` is also single-process and not a substitute.

## What this means in practice

- New behaviour that touches a non-goal is gated on a separate decision,
  captured as a `proposed` requirement (or, for a rejected direction, a
  rationale entry).
- macOS is best-effort, not a contract: a Linux behaviour may ship
  before its macOS equivalent exists.
- Distribution and release engineering (prebuilt artefacts via `cargo
  dist`, a Homebrew tap, a `SCHEMA.md` cheat sheet, `CONTRIBUTING.md`)
  are **not** tracked under `requirements/`. That directory captures
  what the software does, not how it is shipped; those tasks live in the
  issue tracker.
- The single-process scope is why `/proc/<pid>` polling is sufficient
  and a cgroup v2 backend was rejected.

## Related

- [`requirements/`](requirements/) — the contracts that live within this
  scope.
- [`rationale/cgroup-v2-backend-rejected.md`](rationale/cgroup-v2-backend-rejected.md)
  — the single-process scope applied to a concrete backend decision.
- [`requirements/capture-proc-backend.md`](requirements/capture-proc-backend.md)
