---
title: cgroup v2 capture backend
status: planned
---

## Intent

The `/proc` polling backend (see
[`capture-proc-backend`](capture-proc-backend.md)) cannot see processes
that live for less than the sample interval, and it has to walk the
process tree by hand. cgroup v2 solves both problems: creating a
transient cgroup around the child means every descendant — including
short-lived grandchildren — is accounted for automatically by the
kernel, and a single read of `memory.peak` /
`cpu.stat` / `io.stat` returns aggregates over the whole tree.

This is the most accurate capture path for builds and CI workloads where
short-lived helpers (preprocessors, linkers, sub-makes) drive the
numbers, and is named explicitly in the project goals.

## Acceptance criteria (sketch, to firm up before implementation)

- A new `cgroup_v2` value for the `--backend` flag selects this path.
- When picked, rprof creates a transient cgroup, places the child in
  it, and reads `memory.current` / `memory.peak`, `cpu.stat`, and
  `io.stat` instead of `/proc/<pid>`.
- The report's `run.backend` records `cgroup_v2`.
- Short-lived grandchildren that the `/proc` backend would miss are
  reflected in `summary.peak_rss_bytes` and `summary.user_cpu_ms`.
- The cgroup is removed on exit even if the child dies abnormally.
- A graceful fallback to `/proc` is documented when cgroup creation
  fails (missing privileges, no delegation).

## Open questions

- **Privilege model.** Transient cgroups may need either root, a
  delegated subtree (systemd `--user`), or a CAP capability. Pick one
  default and document the others.
- **Hierarchy discovery.** Whether to assume a unified-only system or
  also handle hybrid cgroup mounts.
- **Kernel version floor.** `memory.peak` requires Linux ≥ 5.19; on
  5.15 LTS hosts only `memory.current` is available and peak has to
  be tracked sample-side. Decide whether the backend requires 5.19+
  and refuses to start below that, or falls back to in-process peak
  tracking.
- **Per-sample data shape.** Many `cgroup` files report aggregates,
  not per-process detail. The per-sample schema in
  [`schema-v1`](schema-v1.md) is process-shaped today; decide whether
  the cgroup backend feeds the same fields (aggregates over the
  whole tree) or grows a new variant.

## Notes

- Out of scope for v1 by [`../CLAUDE.md`](../CLAUDE.md)'s non-goals
  framing; intentionally a separate phase because the privilege model
  needs design work.
- Related: [`capture-proc-backend`](capture-proc-backend.md),
  [`capture-process-tree`](capture-process-tree.md).
