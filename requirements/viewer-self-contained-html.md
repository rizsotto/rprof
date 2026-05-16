---
title: Self-contained HTML viewer output
status: implemented
---

## Intent

A user who captures a report on a CI runner wants to view it on their
laptop, in their browser, without installing a Rust toolchain, a Node
runtime, or running a server. The viewer's output must therefore be
self-contained: one HTML file the user can email, attach to a pull
request, or open by double-clicking.

## Acceptance criteria

- `rprof view <report.json>` produces a single HTML file with no
  external references (no `<link rel="stylesheet" href="...">`, no
  `<script src="...">`, no fetch calls to a server).
- The HTML contains the report JSON inlined inside a
  `<script type="application/json" id="rprof-data">` element.
- The HTML contains the uPlot JavaScript bundle and CSS inlined.
- The HTML contains a small viewer JS bundle (rprof's own code) and
  small CSS for the page chrome.
- Opening the file in any modern browser, on a machine with no
  network, renders the charts.
- The default invocation opens the user's browser via `xdg-open`
  (Linux) or `open` (macOS) on a temp file. `--no-open` suppresses
  this and writes to either `-o <path>` or stdout.
- With `--no-open -o <path>`, nothing is printed on stdout.
- With `--no-open` and no `-o`, the HTML goes to stdout. This is the
  shape needed for `rprof view --no-open r.json > out.html` and for
  piping into a clipboard tool.

## Non-functional constraints

- The HTML payload should remain reasonably sized. The uPlot bundle
  is ~50 KiB minified plus ~2 KiB CSS. The total HTML for a small
  report should be under 100 KiB before the inlined data dominates.
- The inlined JSON payload must escape `</` to `<\/` so that a
  command containing literal `</script>` cannot break out of the
  `<script type="application/json">` element. (XSS hardening.)

## Implementation details

- `src/viewer.rs` embeds the four assets via `include_str!` at
  compile time:
  - `assets/uPlot.iife.min.js`
  - `assets/uPlot.min.css`
  - `assets/viewer.js`
  - `assets/viewer.css`
- `render_html()` concatenates a small HTML scaffold with the
  embedded asset strings and the JSON payload.
- The payload is built by `build_payload()` which serialises the
  loaded runs and applies the `</` → `<\/` replacement.
- See [`../assets/CLAUDE.md`](../assets/CLAUDE.md) for licensing
  notes on the bundled assets.

## Known limitations

- The HTML file is not minified. The viewer JS is small enough that
  the file size impact is negligible, and keeping it readable
  helps debugging.
- Browser feature detection is not done; very old browsers without
  `Set`, arrow functions, or `Map` will fail. Modern Edge, Chrome,
  Firefox, and Safari are the support baseline.
- `xdg-open` failure falls back to printing the file path to stderr
  so the user can manually open it.

## Testing

Given a captured report:

> When `rprof view --no-open r.json` runs,
> then `rprof` exits with status 0,
> and its stdout starts with `<!doctype html>`,
> and the output contains the string `uPlot` (the bundled library),
> and the output contains `id="rprof-data"`.

Given the same report and an explicit output path:

> When `rprof view --no-open -o out.html r.json` runs,
> then `out.html` is written,
> and `rprof`'s stdout is empty.

Given a report whose command line contains `</script>`:

> When the HTML is rendered,
> then the literal `</script>` does not appear in the inlined JSON
> payload (it is encoded as `<\/script>`),
> and the page parses correctly in a browser.

## Notes

- The "no Node, no Python, no server" requirement is the central
  v1 acceptance criterion for the viewer. It is why uPlot was
  picked over heavier libraries.
- Related: `viewer-diff-mode` (which builds on this requirement
  for multi-run reports).
