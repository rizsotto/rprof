# CLAUDE.md — `assets/` guide

Compile-time assets embedded into the `rprof` binary via `include_str!` in
[`src/viewer.rs`](../src/viewer.rs). Every file here ships inside every HTML
report `rprof view` emits.

## Files

| File | Origin | Licence |
|---|---|---|
| `uPlot.iife.min.js` | Vendored from upstream uPlot 1.6.31 (https://github.com/leeoniya/uPlot) | MIT (see [`uPlot.LICENSE`](uPlot.LICENSE)) |
| `uPlot.min.css` | Vendored from upstream uPlot 1.6.31 (https://github.com/leeoniya/uPlot) | MIT (see [`uPlot.LICENSE`](uPlot.LICENSE)) |
| `uPlot.LICENSE` | Upstream LICENSE preserved verbatim | MIT |
| `viewer.js` | First-party rprof code | MIT (project licence) |
| `viewer.css` | First-party rprof page styles | MIT (project licence) |

## Rules

### Vendored uPlot files are read-only

Do not hand-edit `uPlot.iife.min.js` or `uPlot.min.css`. To upgrade uPlot:

```bash
VER=1.6.31  # or newer
curl -sSL -o assets/uPlot.iife.min.js https://unpkg.com/uplot@${VER}/dist/uPlot.iife.min.js
curl -sSL -o assets/uPlot.min.css      https://unpkg.com/uplot@${VER}/dist/uPlot.min.css
```

Then:
1. Update the version string in the table above.
2. Re-fetch the upstream LICENSE and overwrite `uPlot.LICENSE` if anything
   shifted (copyright year, holder, identifier).
3. Run `cargo test` — the viewer unit tests check that the bundle string is
   present in the rendered HTML.

The minified bundles ship with only a URL banner and no embedded licence text,
so `uPlot.LICENSE` next to them preserves the upstream copyright and
permission notice as the MIT licence requires.

### First-party files (`viewer.js`, `viewer.css`)

These are MIT-licensed alongside the rest of rprof. They do not carry per-file
SPDX headers; see [`../src/CLAUDE.md`](../src/CLAUDE.md) for the file-header
rules (which apply only to `.rs` files).

### Adding a new asset

Any new file added here must be:
- Either first-party (and MIT-licensed by the project `LICENSE`), or
- A third-party file with a compatible licence (MIT, Apache-2.0, BSD).

If you add a new third-party asset, add an entry to the table above
recording its upstream URL, version, and licence. Wire it into
`src/viewer.rs` via `include_str!` and pin a test in `src/viewer.rs` that
asserts the new bundle appears in the rendered HTML.

## Size budget

The whole `assets/` tree currently adds ~52 KiB to the release binary. The
v1 acceptance criterion is "release tarball under 10 MB"; the actual binary
is ~1.3 MB. There is room to grow, but new assets should be justified — the
self-contained HTML report inherits the same weight.
