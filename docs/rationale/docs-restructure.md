# Separate contracts, rationale, and CLI surface

## Context

rprof's documentation served three distinct roles, but two of them
were tangled inside `requirements/`:

- **Contracts** — what the user can expect — lived correctly in the
  requirement files, verified by tagged tests.
- **Rationale** — why a design was chosen — had no home. It leaked into
  the `## Implementation details` section of requirements (the template
  even described that section as the place that "captures the
  rationale") and into Notes.
- **User-facing CLI surface** — flags, defaults — is documented in the
  top-level `README.md` and pinned by integration tests, but was
  partly duplicated inside requirement bodies.

The tangle had two costs. A reader could not tell a binding contract
from the reasoning behind it. And one whole file,
`capture-cgroup-v2-backend.md`, was a `status: rejected` exploration
with no acceptance criteria — a decision record misfiled as a
contract.

## Decision

Give each role one home and one drift-control mechanism:

| Role | Home | Drift control |
|---|---|---|
| Contract | `docs/requirements/*.md` | `Requirements:` tags + coverage check |
| Rationale | `docs/rationale/*.md` | linked from the requirement it supports |
| CLI surface | `README.md` + `capture-cli-contract` | integration tests |

Requirements become contract-only: the `## Implementation details`
section is removed everywhere, with its content routed to a rationale
entry (the why) or a code comment (the how). A new optional
`## Rationale` section links a requirement to the entries that
motivated it.

We chose separate rationale files over keeping a free-form rationale
section in each requirement because rationale and contract change on
different clocks and want different review: a contract change can break
tests, a rationale edit cannot. Keeping them in one file invited
exactly the drift this restructure removes.

Rationale files use descriptive kebab-case names with no number prefix,
matching requirements; git history supplies chronology.

Deliberately **not** adopted from the plan this work was adapted from
(written for the Bear project): a `configuration.md` and config sync
check — rprof has no configuration file, only CLI flags — and a
generated man page — rprof ships none, and adding one is a feature, not
a docs move.

## Consequences

- An agent or human can find the binding contract without wading
  through reasoning, and find the reasoning without re-deriving it from
  a PR thread.
- New design decisions and rejected alternatives now have an obvious
  destination; [`cgroup-v2-backend-rejected`](cgroup-v2-backend-rejected.md)
  records the first one.
- A new convention to uphold: a requirement may carry a `## Rationale`
  link, and decisions only ever considered and declined are written as
  rationale entries, not as `rejected` requirements.

## References

- Supports the documentation contract described across the repo's
  `CLAUDE.md` files.
- [`cgroup-v2-backend-rejected.md`](cgroup-v2-backend-rejected.md) — the
  first rationale entry produced under this structure.
