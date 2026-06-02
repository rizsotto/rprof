# CLAUDE.md — `docs/` guide

This is the index for project documentation — start here to find what
each part holds.

The two role-based subdirectories:

| Directory | Holds | Guide |
|---|---|---|
| [`requirements/`](requirements/) | Contracts: what `rprof` must do, from the user's perspective. Verified by tagged tests. | [`requirements/CLAUDE.md`](requirements/CLAUDE.md) |
| [`rationale/`](rationale/) | Decision records: the reasoning behind a design choice (or a rejected alternative). | [`rationale/CLAUDE.md`](rationale/CLAUDE.md) |

Keep the roles separate. A requirement says *what*; a rationale entry
says *why*; the code says *how*. Reasoning does not belong in a
requirement body, and a contract does not belong in a rationale entry.

Standalone reference documents directly under `docs/`:

- [`project-scope.md`](project-scope.md) — the project's goals and
  deliberate non-goals; the boundary every feature is weighed against.
- [`architecture.md`](architecture.md) — how data flows through the two
  subcommands, end to end.

`docs/` holds reference documentation. Operational procedures (releasing,
and similar) are invocable skills under
[`../.claude/skills/`](../.claude/skills/), not files here.

The user-facing CLI reference — flags, defaults, examples — is the
top-level [`README.md`](../README.md), kept honest by the integration
tests in [`../tests/`](../tests/).
