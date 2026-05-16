# CLAUDE.md — `requirements/` guide

This directory is the source of truth for what `rprof` is supposed to do.
Tests verify that the implementation matches; code that drifts from these
specs is wrong, not the spec.

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
omit `Non-functional constraints`, `Implementation details`, `Known
limitations`, and `Testing` when those are still being decided — capture
the unknowns under an `Open questions` block in `Notes` instead.

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

## Implementation details

Key choices and why. Keep brief; the code is the source of truth for the
how, this section captures the rationale.

## Known limitations

What this requirement intentionally does **not** cover. Helps reviewers
catch out-of-scope feature creep.

## Testing

Given-When-Then scenarios. These are the canonical scenarios; integration
tests in `tests/` implement them.

## Notes

Decisions, links to issues, future work.
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

1. **Before adding a feature**: search for an existing requirement. If none
   exists, draft one with `status: proposed` and stop — wait for user
   approval before writing code.
2. **Before changing behaviour**: find the governing requirement and read
   its acceptance criteria. Those are the contracts that must not break.
3. **After implementing**: flip the requirement to `status: implemented`
   and add `Requirements: <id>` tags to the protecting test(s).
4. **When fixing a regression**: write a test that reproduces the bug, tag
   it with the requirement ID that the regression violated, then fix the
   code. The tagged test is the regression's tombstone.

## Coverage check

There is currently no automated coverage script — adding one is tracked
informally and would belong here as `check-coverage.sh`. The manual check is:

```bash
for f in requirements/*.md; do
  id=$(basename "$f" .md)
  [ "$id" = "CLAUDE" ] && continue
  status=$(awk '/^status:/{print $2; exit}' "$f")
  [ "$status" = "implemented" ] || continue
  if ! grep -rq "Requirements:.*\b${id}\b" src/ tests/; then
    echo "no tests tag implemented requirement: $id"
  fi
done
```

## Things that do NOT belong here

- Step-by-step implementation guides — those go in code comments or the
  per-directory `CLAUDE.md` files.
- Roadmap, goals, or non-goals — those live in
  [`../CLAUDE.md`](../CLAUDE.md) under "Project overview".
- Bug reports or to-dos — those belong in the issue tracker.

The requirements directory captures **what the software must do**, not how
it is built and not what we wish it did some day.
