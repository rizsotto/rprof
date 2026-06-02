---
title: Time-axis alignment for diff mode
status: planned
---

## Intent

When the user diffs two runs of unequal wall duration, the longer run
fills the chart while the shorter run looks like a short bump at the
left. For some comparisons that is correct ("the new version finished in
half the time"); for others, the user wants to see the metrics as a
fraction of each run's wall time so the shapes line up. A `--align`
flag lets them pick.

## Acceptance criteria (sketch, to firm up before implementation)

- A new `--align <mode>` flag on `rprof view` accepts three values:
  - `none` (default; absolute per-run `t_ms` X axis, current v1
    behaviour).
  - `wall` (each run's X axis stretched to percent-of-wall-time so
    runs of different durations line up).
  - `epoch` (X axis is each sample's `wall_ms`, so two runs captured
    at overlapping wall-clock times align on the real timeline).
- The default remains `none` to keep current scripts working.
- The chosen mode is reflected in the viewer JS by mapping per-sample
  `t_ms` to `t_ms` (`none`), `t_ms / wall_duration_ms * 100`
  (`wall`), or `wall_ms` (`epoch`).
- The X-axis label switches to match: `time (s)` for `none`,
  `wall %` for `wall`, wall-clock formatted time for `epoch`.
- The summary table is unchanged; alignment only affects chart axes.

## Open questions

- **Mixed-mode comparisons.** Should `--align wall` be allowed with a
  single run, or rejected as nonsensical?
- **Crosshair semantics.** With normalisation, the shared cursor's
  reading on each run is at a different absolute `t_ms`. Document the
  interaction.

## Notes

- v1 ships with `none`-style absolute milliseconds only; the
  acceptance criteria for [`viewer-diff-mode`](viewer-diff-mode.md)
  reflect that intentional limitation.
