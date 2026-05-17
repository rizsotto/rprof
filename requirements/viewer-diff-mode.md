---
title: Multi-run diff mode
status: implemented
---

## Intent

Comparing two runs (before/after, slow/fast, debug/release) is the most
common reason a user reaches for a profiler. `rprof view a.jsonl b.jsonl`
must overlay both runs on every chart with distinct colours and a working
legend, so the user can read the difference visually instead of squinting
at two browser tabs.

## Acceptance criteria

- `rprof view r1.jsonl r2.jsonl [r3.jsonl ...]` accepts any number of
  reports (subject to `argv` length).
- Each run is plotted as its own series in every chart, with a stable
  per-run colour from a fixed palette.
- The summary table above the charts has one row per run, comparing
  command, wall duration, peak RSS, CPU time, and exit code. Each
  row's label cell carries a colour swatch matching that run's chart
  series.
- `--label LABEL:PATH` overrides the default filename-based label for
  that report. `--label` can be repeated; the last one wins on
  conflict.
- When `--label LABEL:PATH` names a path that is not also a positional
  argument, the path *is* loaded — `--label` alone is sufficient to
  specify a report. This makes
  `rprof view --label before:a.jsonl --label after:b.jsonl` a valid
  invocation.
- When no `--label` is given for a positional, the file's stem (the
  filename without the extension) is the default label.

### Colour palette

- The palette is a fixed cycle of 8 hues (Tableau-derived): blue,
  orange, green, red, purple, brown, pink, gray. Each hue has a
  **dark** and **light** variant: dark for the primary metric on a
  multi-series chart (RSS, user CPU, IO read), light for the
  secondary on the same chart (VSZ, system CPU, IO write). Same hue
  per run keeps each run's series visually grouped across charts.
- A run's palette index is its position in the loaded list (modulo
  palette length). Re-ordering inputs changes colours; this is by
  design — the colour is determined by argument order, not content.

### Cursor snapping with sparse overlays

- Different runs sample at different absolute timestamps, so the
  X-axis union of `t_ms` values has many positions where only one
  run has a real value. Without intervention, hovering at one of
  those X positions would show "value / —" for the other runs.
- The cursor snaps **per series** to that series' nearest non-null
  sample (tie-break: prefer the earlier index). So at any hovered X
  the legend reads a real value for every run, even if those values
  came from slightly different absolute timestamps.

## Non-functional constraints

- The X axis is in absolute milliseconds (run-local) by default. Runs
  of unequal duration are not normalised; the longer run simply has a
  longer X domain. The future `--align` flag (see
  [`viewer-x-axis-align`](viewer-x-axis-align.md)) will normalise to
  percent-of-wall-time.
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
- Wall-clock alignment (`--align`) is planned future work; see
  [`viewer-x-axis-align`](viewer-x-axis-align.md).
- Two reports captured at very different sample intervals will look
  jagged when overlaid; the X axis union does not interpolate.

## Testing

Given two captured reports:

> When `rprof view --no-open -o out.html a.jsonl b.jsonl` runs,
> then `out.html` contains both runs' inlined data,
> and the page title contains "2 runs".

Given two reports with `--label`:

> When the user runs
> `rprof view --no-open -o out.html --label before:a.jsonl --label after:b.jsonl`,
> then the inlined payload contains `"label":"before"` and
> `"label":"after"`,
> and the summary table renders with those labels.

Given a single positional report with no `--label`:

> When the user runs `rprof view --no-open -o out.html my-build.jsonl`,
> then the inlined payload contains `"label":"my-build"` (the file
> stem).

## Notes

- The decision to colour by run and use the light/dark variant per
  metric keeps each chart readable even with two or three runs.
  Beyond 4-5 runs the user should consider breaking the comparison
  into pairs.
- Cross-panel cursor sync (one hover shows readings on every chart at
  the same X) and live-legend stability are not diff-mode-specific —
  they apply to single-run views too. Those behaviours live in
  [`viewer-chart-interaction`](viewer-chart-interaction.md).
- The chart inventory (which metrics share each chart, and the
  primary/secondary convention) is in
  [`viewer-chart-inventory`](viewer-chart-inventory.md).
- Related: `viewer-self-contained-html`.
