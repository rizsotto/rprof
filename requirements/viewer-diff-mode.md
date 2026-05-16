---
title: Multi-run diff mode
status: implemented
---

## Intent

Comparing two runs (before/after, slow/fast, debug/release) is the most
common reason a user reaches for a profiler. `rprof view a.json b.json`
must overlay both runs on every chart with distinct colours and a working
legend, so the user can read the difference visually instead of squinting
at two browser tabs.

## Acceptance criteria

- `rprof view r1.json r2.json [r3.json ...]` accepts any number of
  reports (subject to `argv` length).
- Each run is plotted as its own series in every chart, with a stable
  per-run colour from a fixed palette.
- The summary table above the charts has one row per run, comparing
  command, wall duration, peak RSS, CPU time, and exit code.
- `--label LABEL:PATH` overrides the default filename-based label for
  that report. `--label` can be repeated; the last one wins on
  conflict.
- When `--label LABEL:PATH` names a path that is not also a positional
  argument, the path *is* loaded — `--label` alone is sufficient to
  specify a report. This makes
  `rprof view --label before:a.json --label after:b.json` a valid
  invocation.
- When no `--label` is given for a positional, the file's stem (the
  filename without the `.json` extension) is the default label.
- The legend names each series by its run label, optionally suffixed
  with the metric variant (`user` / `sys` for CPU; `RSS` / `VSZ` for
  memory; `read` / `write` for IO).
- The shared cursor crosshair syncs across panels so the user reads
  every metric for the same timestamp at once.

## Non-functional constraints

- The X axis is in absolute milliseconds (run-local) by default. Runs
  of unequal duration are not normalised; the longer run simply has a
  longer X domain. A future `--align` flag for percent-of-wall-time
  alignment is mentioned in `idea.md` Phase 3.
- Up to ~8 runs render cleanly with the default palette. Beyond that,
  colours wrap around and the chart becomes hard to read; the legend
  still works correctly.

## Implementation details

- `collect_inputs()` in `src/viewer.rs` merges positional args and
  `--label` entries, preserving positional order and appending
  labels-only entries afterward.
- `render_html()` and the viewer JS in `assets/viewer.js` walk the
  `runs` array in the inlined payload to build the summary table and
  one or more uPlot series per metric.
- The X axis is built as the sorted union of all `t_ms` values
  across runs. Each run's Y array is `null`-padded for X values it
  does not have, so uPlot's gap rendering does the right thing.

## Known limitations

- No semantic diffing (e.g. "RSS grew by 12 %") is performed; the
  diff is purely visual.
- Wall-clock alignment (`--align`) is deferred to phase 3.
- Two reports captured at very different sample intervals will look
  jagged when overlaid; the X axis union does not interpolate.

## Testing

Given two captured reports:

> When `rprof view --no-open -o out.html a.json b.json` runs,
> then `out.html` contains both runs' inlined data,
> and the page title contains "2 runs".

Given two reports with `--label`:

> When the user runs
> `rprof view --no-open -o out.html --label before:a.json --label after:b.json`,
> then the inlined payload contains `"label":"before"` and
> `"label":"after"`,
> and the summary table renders with those labels.

Given a single positional report with no `--label`:

> When the user runs `rprof view --no-open -o out.html my-build.json`,
> then the inlined payload contains `"label":"my-build"` (the file
> stem).

## Notes

- The decision to colour by run and dash by metric variant keeps each
  chart readable even with two or three runs. Beyond 4-5 runs the
  user should consider breaking the comparison into pairs.
- Related: `viewer-self-contained-html`.
