# CLAUDE.md — `docs/` guide

This is the index for project documentation — start here to find what
each part holds. Today that is the contracts and the reasoning behind
them; as the project grows, operational how-to guides (releasing,
cutting a schema revision, and similar) will be written here too, each
as its own file under `docs/`.

Documentation is split by the role each file plays:

| Directory | Holds | Guide |
|---|---|---|
| [`requirements/`](requirements/) | Contracts: what `rprof` must do, from the user's perspective. Verified by tagged tests. | [`requirements/CLAUDE.md`](requirements/CLAUDE.md) |
| [`rationale/`](rationale/) | Decision records: the reasoning behind a design choice (or a rejected alternative). | [`rationale/CLAUDE.md`](rationale/CLAUDE.md) |

Keep the roles separate. A requirement says *what*; a rationale entry
says *why*; the code says *how*. Reasoning does not belong in a
requirement body, and a contract does not belong in a rationale entry.

Two cross-cutting references worth knowing:

- [`architecture.md`](architecture.md) — how data flows through the two
  subcommands, end to end.
- [`rationale/project-scope.md`](rationale/project-scope.md) — the
  project's goals and deliberate non-goals.

The user-facing CLI reference — flags, defaults, examples — is the
top-level [`README.md`](../README.md), kept honest by the integration
tests in [`../tests/`](../tests/).
