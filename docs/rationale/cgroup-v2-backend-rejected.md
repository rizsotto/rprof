# cgroup v2 capture backend, rejected

## Context

A cgroup v2 backend was considered as a more accurate alternative to
`/proc` polling. Creating a transient cgroup around the child would let
the kernel account for every descendant, including short-lived
grandchildren the sampler can miss between ticks, and would expose
tree-wide aggregates directly via `memory.peak`, `cpu.stat`, and
`io.stat` rather than deriving them from periodic `/proc/<pid>/stat`
reads.

The motivation was almost entirely **tree aggregation**: capturing the
whole process subtree rather than just the spawned child.

## Decision

Rejected. Tree aggregation is an explicit non-goal (see the Non-goals
section of the root [`CLAUDE.md`](../../CLAUDE.md)): rprof reports the
resource usage of the single child it spawned, and summing across a
tree double-counts shared memory and FDs while hiding which descendant
drove a spike. For that single-process scope, `/proc/<pid>` polling is
sufficient -- `memory.peak` and per-tree `cpu.stat` are not features we
need.

The remaining benefit, slightly better single-process accuracy, did not
justify the cost: a transient cgroup requires elevated privilege or
delegation (root, systemd `--user` delegation, or a `CAP_*` capability),
which conflicts with the "drop the static binary into any container and
it works" goal.

## Consequences

- The `/proc` polling backend remains the only capture path on Linux;
  see [`../requirements/capture-proc-backend.md`](../requirements/capture-proc-backend.md).
- If a future contributor wants per-process accuracy improvements beyond
  `/proc/<pid>/stat`, that is a fresh proposal -- scoped to
  single-process metrics, not tree aggregation, and weighed against the
  no-privilege goal again on its own terms.

## References

- [`../requirements/capture-proc-backend.md`](../requirements/capture-proc-backend.md)
  -- the backend kept instead.
- Non-goals section of the root [`CLAUDE.md`](../../CLAUDE.md)
  (process-tree resource aggregation).
