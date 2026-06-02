---
title: cgroup v2 capture backend
status: rejected
---

## Intent

A cgroup v2 backend was considered as a more accurate alternative to
`/proc` polling: creating a transient cgroup around the child would
let the kernel account for every descendant (including short-lived
grandchildren) and would expose tree-wide aggregates via
`memory.peak` / `cpu.stat` / `io.stat`.

## Notes

- **Rejected.** The motivation was almost entirely tree aggregation,
  which is now a non-goal (see the Non-goals section in
  [`../CLAUDE.md`](../CLAUDE.md)). For the single-process scope `rprof`
  targets, `/proc/<pid>` polling is sufficient: `memory.peak` and
  per-tree `cpu.stat` are no longer features we need, and the
  privilege/delegation work a transient cgroup requires (root,
  systemd `--user` delegation, or a `CAP_*` capability) is not worth
  carrying for what would remain of the backend.
- If a future contributor wants per-process accuracy improvements
  beyond `/proc/<pid>/stat`, a fresh proposal — scoped to
  single-process metrics, not tree aggregation — is the right path.
  This file is kept as a tombstone so the rejection rationale is
  discoverable.
- Related: [`capture-proc-backend`](capture-proc-backend.md).
