# CLAUDE.md — `requirements/` guide

This directory is the source of truth for what `rprof` is supposed to do.
Tests verify that the implementation matches; code that drifts from these
specs is wrong, not the spec.

Requirements are **contract-only**: they describe what the user can
expect, not why a design was chosen and not where the bits live. The
*why* belongs in [`../rationale/`](../rationale/) (link it from a
`## Rationale` section); the *how* belongs in the code and its comments.

## File naming

```
<area>-<short-name>.md
```

The filename without the `.md` extension is the requirement's unique ID. Use
it in cross-references between requirement files and in `Requirements:` tags
on tests.

Areas in use today:

| Area | Scope |
|---|---|
| `capture-*` | Behaviour of `rprof run` (the capture subcommand). |
| `schema-*` | The on-disk JSON report format. |
| `viewer-*` | Behaviour of `rprof view` (rendering, diff mode, output). |

## Template

Every requirement file starts with YAML frontmatter and follows the
structure below. Files at `status: implemented` should fill every
section; `proposed`, `accepted`, and `planned` files may legitimately
omit `Non-functional constraints`, `Known limitations`, and `Testing`
when those are still being decided — capture the unknowns under an
`Open questions` block in `Notes` instead.

```markdown
---
title: Short human-readable title
status: implemented
---

## Intent

What the user expects, written from the user's perspective.

## Acceptance criteria

- Concrete, measurable bullet points.

## Non-functional constraints

Performance, platform support, etc. Omit if not relevant.

## Known limitations

What this requirement intentionally does **not** cover. Helps reviewers
catch out-of-scope feature creep.

## Testing

Given-When-Then scenarios. These are the canonical scenarios; integration
tests in `tests/` implement them.

## Notes

Brief decisions, links to issues, future work. A one-line decision is
fine here; substantial reasoning or a rejected alternative goes in a
rationale entry instead, linked below.

## Rationale

Optional. A list of links to the rationale entries under
[`../rationale/`](../rationale/) that motivated this requirement — one
short label per link, no prose. Omit the section when there is nothing
to link.
```

## Status lifecycle

| Status | Meaning |
|---|---|
| `proposed` | Captured but not reviewed; awaits user agreement. |
| `accepted` | Reviewed and approved; ready for implementation. |
| `planned` | Approved as future work; documented now so the spec is ready when picked up. No work currently underway. |
| `in-progress` | Implementation underway. |
| `implemented` | Code complete, tests passing. |
| `deferred` | Accepted but intentionally postponed indefinitely (give the reason in Notes). |
| `rejected` | Reviewed and declined (give the reason in Notes). |

## Linking tests to requirements

Tests that protect a requirement carry a `Requirements:` comment
immediately above the `#[test]` attribute (or `#![cfg(...)]` block, for
whole-file tags).

```rust
// Requirements: capture-signal-forwarding
#[test]
fn run_forwards_sigint_and_still_writes_report() { ... }
```

Rules:

- Value is a comma-separated list of requirement IDs (filenames without `.md`).
- The tag lives in the test source. Renaming or deleting the test updates the
  link in the same edit, so the trace cannot rot.
- For a file that covers a single requirement, place
  `//! Requirements: <id>` inside the top-of-file module doc-comment
  block instead of tagging every `#[test]`. Note: rprof's current test
  files cover multiple requirements each, so they use per-test
  comments rather than file-level tags. The file-level form is
  documented for the case where a future file is dedicated to a
  single requirement.
- When both a file-level and a test-level tag are present, treat them
  as additive (the test is covered by the union of both ID lists).

### Finding tests for a requirement

```bash
grep -rn "Requirements:.*capture-signal-forwarding" src/ tests/
```

## How agents should use this directory

1. **Before adding a feature**: search for an existing requirement.
   - If none exists, draft one with `status: proposed` and stop — wait
     for user approval before writing code.
   - If one exists at `status: planned` and its Notes still list
     **Open questions**, stop and resolve those with the user first.
     Promote the requirement to `accepted` (with the open questions
     answered) before writing any code. A `planned` spec is by
     definition incomplete; treating it as a green light leads to
     implementations that miss the still-undecided parts.
2. **Before changing behaviour**: find the governing requirement and read
   its acceptance criteria. Those are the contracts that must not break.
3. **After implementing**: flip the requirement to `status: implemented`
   and add `Requirements: <id>` tags to the protecting test(s).
4. **When fixing a regression**: write a test that reproduces the bug, tag
   it with the requirement ID that the regression violated, then fix the
   code. The tagged test is the regression's tombstone.

## Coverage check

Every `status: implemented` requirement must have at least one tagged
test. CI enforces this via the lint job; you can run the same check
locally:

```sh
sh scripts/check-requirements-coverage.sh
```

The script exits 0 when every implemented requirement is covered and
exits 1 listing the gaps otherwise. Requirements at any other status
(`proposed`, `accepted`, `planned`, `in-progress`, `deferred`,
`rejected`) are skipped — they may not have tests yet, by design.

## Things that do NOT belong here

- Step-by-step implementation guides — those go in code comments or the
  per-directory `CLAUDE.md` files.
- Design rationale, trade-offs, or rejected alternatives — those go in
  [`../rationale/`](../rationale/); link them from a `## Rationale`
  section.
- Roadmap, goals, or non-goals — those live in
  [`../../CLAUDE.md`](../../CLAUDE.md) under "Project overview".
- Bug reports or to-dos — those belong in the issue tracker.

The requirements directory captures **what the software must do**, not how
it is built, not why we built it that way, and not what we wish it did
some day.
