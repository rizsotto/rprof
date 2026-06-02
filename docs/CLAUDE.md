# CLAUDE.md — `docs/` guide

Project documentation lives here, split by the role each file plays:

| Directory | Holds | Guide |
|---|---|---|
| [`requirements/`](requirements/) | Contracts: what `rprof` must do, from the user's perspective. Verified by tagged tests. | [`requirements/CLAUDE.md`](requirements/CLAUDE.md) |
| [`rationale/`](rationale/) | Decision records: the reasoning behind a design choice (or a rejected alternative). | [`rationale/CLAUDE.md`](rationale/CLAUDE.md) |

Keep the roles separate. A requirement says *what*; a rationale entry
says *why*; the code says *how*. Reasoning does not belong in a
requirement body, and a contract does not belong in a rationale entry.

The user-facing CLI reference — flags, defaults, examples — is the
top-level [`README.md`](../README.md), kept honest by the integration
tests in [`../tests/`](../tests/).
