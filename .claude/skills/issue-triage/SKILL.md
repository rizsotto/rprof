---
name: issue-triage
description: Triage a GitHub issue for rprof — classify it against project scope and the requirements, decide the next action (bug fix, new requirement, rejected direction, or question), and propose labels. Use when the user asks to triage an issue, label issues, or work through the issue backlog.
---

# Triage an rprof issue

Classify a GitHub issue and decide what happens to it, using rprof's own
docs as the rubric. **Reading is free; confirm with the user before any
write to the public repo** — posting a comment, applying labels, or
closing an issue.

## Gather

1. Read the issue: `gh issue view <number> --comments`.
2. List the repo's existing labels so you reuse them rather than invent:
   `gh label list`.
3. If it is a bug claim with a reproduction, try to reproduce it (build
   and run `rprof` per the repo's CLAUDE.md) before classifying.

## Classify

Work the checks in this order — the cheapest filter first.

1. **Out of scope?** Compare against the non-goals in
   `docs/project-scope.md` (flamegraphs/stack sampling, distributed
   tracing, GPU/network/syscall accounting, Windows, a daemon, or
   process-tree aggregation). If it asks for one of these, it is a
   decline, not a backlog item. Cite the relevant non-goal. If the same
   direction keeps being requested, suggest capturing the rejection as a
   rationale entry under `docs/rationale/` so the reasoning is reusable.
   → proposed label: the repo's `wontfix` / out-of-scope equivalent.

2. **Bug — a broken contract?** Find the governing requirement in
   `docs/requirements/`. If `rprof`'s behaviour violates that
   requirement's acceptance criteria, it is a bug. The fix path is
   test-first: a failing integration test tagged
   `// Requirements: <id>`, then the code change (see the root
   `CLAUDE.md` decision protocol).
   → proposed label: `bug`. Note the requirement ID in the triage.

3. **New behaviour, in scope?** If it is a feature or behaviour change
   that no requirement covers yet and no non-goal forbids, it is
   requirement-first: a `proposed` requirement must be written and
   agreed before any code (see `docs/requirements/CLAUDE.md`).
   → proposed label: `enhancement`. Next step: draft the requirement.

4. **Usage or question?** Answer it; point at the top-level `README.md`
   or the relevant doc. No code change.
   → proposed label: `question`.

5. **Documentation gap?** A wrong or missing doc, not a behaviour change.
   → proposed label: `documentation`.

## Propose, then act

Summarise for the user, per issue:

- Classification and the evidence (the non-goal, requirement ID, or doc
  it maps to).
- The next concrete action (decline with rationale / write a failing
  test / draft a `proposed` requirement / answer / fix a doc).
- The label(s) to apply, drawn from `gh label list`.

After the user confirms, apply with `gh`:
`gh issue edit <number> --add-label "<label>"`, and post the triage
summary as a comment with `gh issue comment` if they want it recorded.
Do not close an issue as out-of-scope without explicit confirmation.
