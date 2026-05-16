---
title: Viewer chart interaction (collapse, cursor sync, stable legend)
status: implemented
---

## Intent

The viewer is a single HTML file, but it is interactive: the user
reads several metrics at once by hovering, hides charts they don't
care about, and expects numbers in the legend not to dance around as
the cursor moves. These three concerns — collapse, cross-panel cursor
sync, legend stability — are general to *any* viewer page, whether
it shows one run or six, so they get their own requirement.

Chart contents and per-run colouring belong elsewhere:
[`viewer-chart-inventory`](viewer-chart-inventory.md) and
[`viewer-diff-mode`](viewer-diff-mode.md).

## Acceptance criteria

### Per-chart layout

Every chart panel is one self-contained block on the page, in this
internal order:

1. **Title row.** Contains the chart's human name (e.g. "CPU %",
   "Memory") on the left and a **collapse toggle button** on the
   right.
2. **Plot canvas.** The uPlot-rendered chart itself.
3. **Legend.** A live legend that updates as the cursor moves;
   immediately below the plot canvas.

These three elements visually belong together: a single bordered card
per chart, not three free-floating elements.

### Collapse / expand

- Clicking the collapse toggle hides everything in the panel **except**
  the title row. The button label flips from `−` to `+`, its
  `aria-label` flips from `Hide chart` to `Show chart`. Clicking
  again restores the panel.
- Collapsed state is per-chart, independent of other charts.
- Collapsed state is **not** persisted: a page reload starts every
  chart expanded.

### Cross-panel cursor sync

- Moving the pointer over any chart shows a vertical crosshair on
  that chart at the pointer's X. **At the same time**, every other
  visible chart shows its own crosshair at the matching X position.
  The user can read CPU, memory, threads, FDs, and IO at one
  timestamp without moving the pointer.
- Sync is by X value (time), not by sample index. Runs that sampled
  at different intervals still snap correctly.
- Collapsed charts are not updated visually, but their state does not
  break the sync on others.

### Legend value stability

- Legend values update **live** as the cursor moves, but the legend's
  *layout* must not jitter. Specifically:
  - Numeric values render with tabular figure widths (digit `0`
    occupies the same width as `1`, `2`, …).
  - Each value cell has a fixed minimum width tuned to the typical
    worst-case reading for that chart (e.g. wider for Memory and IO
    rate than for Threads).
- A value cell grows beyond its minimum only for genuine outliers,
  and only that one cell — neighbouring cells do not shift.

### Single-run vs multi-run

All four behaviours above apply identically whether the page shows
one run or many.

## Non-functional constraints

- No JavaScript framework: the viewer is hand-rolled JS over uPlot.
  Adding a framework would balloon the inlined-asset size and
  contradict [`viewer-self-contained-html`](viewer-self-contained-html.md).
- A11y: the collapse toggle is a real `<button>` with an
  `aria-label`. Tab order and keyboard activation work.

## Implementation details

- Cross-panel sync uses uPlot's `cursor.sync` with a shared key for
  every chart on the page.
- Per-series snap-to-nearest-non-null (a related cursor concern
  specific to sparse multi-run overlays) is described in
  [`viewer-diff-mode`](viewer-diff-mode.md); it does not affect the
  sync semantics described here.
- Legend stability is delivered by CSS: `font-variant-numeric:
  tabular-nums` on the legend container, plus a per-chart
  `min-width` on the value cell. The widths are tuned in
  `assets/viewer.css`.
- The collapse button is a real `<button>`; the collapsed state is a
  CSS class on the chart panel that hides everything except the
  title row via `:not(.chart-title) { display: none }`.

## Known limitations

- No "collapse all" / "expand all" shortcut.
- No keyboard shortcut to move the cursor along the X axis (the
  cursor follows the pointer only).
- Collapse state is not remembered across reloads. Persisting would
  need either URL hash state or local storage; deferred until the
  pain is real.

## Testing

The current automated tests (in `src/viewer.rs`) cover the rendered
HTML structure (presence of chart containers and inlined payload).
The behaviours above are observable in a browser but not currently
exercised by an automated UI test; the in-tree dogfood script
(`scripts/dogfood.sh`) is the canonical way to eyeball them.

Given a rendered HTML:

> When the file is opened in a modern browser,
> then each chart panel shows a title with a collapse toggle, a
> canvas, and a legend;
> and clicking a toggle collapses or restores that panel without
> affecting the others;
> and hovering over any chart shows a synchronised crosshair on every
> other chart;
> and moving the cursor does not visibly shift legend column widths.

## Notes

- Adding a UI test (headless browser, screenshot diff, or jsdom-based
  unit tests of the viewer JS) is recognised as worth doing but not
  yet planned. If a UI regression slips past dogfooding, that is the
  prompt to plan it.
- Related: [`viewer-chart-inventory`](viewer-chart-inventory.md),
  [`viewer-diff-mode`](viewer-diff-mode.md),
  [`viewer-self-contained-html`](viewer-self-contained-html.md).
