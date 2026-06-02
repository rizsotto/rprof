---
title: Viewer chart inventory and per-chart contents
status: implemented
---

## Intent

The viewer's value comes from showing the *right metric on the right
chart at the right place on the page*. This requirement enumerates
which charts the page contains, what each one plots, and in what
order — so a reimplementation (or a redesign) cannot quietly drop a
metric, reorder the panels into something unfamiliar, or change the
primary/secondary metric convention.

Interaction concerns (collapsibility, cursor sync, legend stability)
live in [`viewer-chart-interaction`](viewer-chart-interaction.md);
multi-run colouring lives in [`viewer-diff-mode`](viewer-diff-mode.md).

## Acceptance criteria

### Page structure

- The page renders, top to bottom:
  1. A page title (`rprof — <label>` for one run, `rprof — N runs`
     otherwise) and a subtitle line stating schema version, sampling
     backend, and sample interval.
  2. A **summary table** with one row per run, with columns: Run
     label (carrying a colour swatch), Command, Wall duration, Peak
     RSS, User CPU, System CPU, Exit. Exit is either the numeric exit
     code, `signal N`, or `—` (none of those known yet).
  3. Five chart panels, in this fixed order: CPU, Memory, Threads,
     Open file descriptors, IO rate.

### Chart contents

For every chart, the X axis is **run-local time in seconds**, derived
from the per-sample `t_ms / 1000`. Run-local means each run's first
sample is at `x = 0`; runs of different absolute start times are not
shifted to a common epoch (see `viewer-x-axis-align` for the planned
alignment flag).

Each chart's series follow the per-run colouring rule from
`viewer-diff-mode`: when a chart has two series per run (primary and
secondary), the **primary** uses the run's dark variant and the
**secondary** uses the light variant.

| Chart   | Y-axis unit | Series per run                                                            |
|---------|-------------|---------------------------------------------------------------------------|
| CPU     | percent     | `user` (primary, from `cpu_user_pct`) + `sys` (secondary, from `cpu_sys_pct`) |
| Memory  | bytes       | `RSS` (primary, from `rss_bytes`) + `VSZ` (secondary, from `vsz_bytes`)   |
| Threads | count       | one series (from `threads`)                                                |
| Open file descriptors | count | one series (from `open_fds`)                                       |
| IO rate | bytes / second | `read` (primary) + `write` (secondary), see derivative rule below     |

### Series naming in the legend

Each series is labelled with the run label, optionally suffixed with
the metric variant. For multi-series charts the suffixes are: `user` /
`sys` for CPU, `RSS` / `VSZ` for Memory, `read` / `write` for IO. For
the two single-series charts (Threads, Open file descriptors) the run
label appears unadorned.

### IO rate derivative

- The schema stores **cumulative** `io_read_bytes` and
  `io_write_bytes`. The viewer renders **instantaneous rate**.
- For sample `i ≥ 1` of a given run, the read rate is
  `(io_read_bytes[i] - io_read_bytes[i-1]) / ((t_ms[i] - t_ms[i-1]) / 1000)`,
  clamped to `≥ 0`. The clamp guards against non-monotonic counters
  (`/proc/<pid>/io` is not guaranteed monotonic in pathological
  cases). Same formula for write.
- The first sample of each run has read rate and write rate `0` — no
  previous sample exists to delta against.

## Non-functional constraints

- Numeric values in the legend and summary table use SI-ish byte units
  (`B`, `KiB`, `MiB`, `GiB`) and short duration units (`ms`, `s`,
  `min`). Counts use locale grouping. Percents render with one
  decimal place.
- Dropping a chart or adding a new one is a contract change. Adding
  must be a deliberate edit to this file plus the renderer plus a
  test.

## Known limitations

- The chart order is fixed; the user cannot reorder or hide charts
  via configuration (see `viewer-chart-interaction` for the
  collapse-at-runtime affordance).
- IO rate clamping to `≥ 0` means a counter that legitimately resets
  to zero mid-run (very rare) shows a `0` rate for one sample
  instead of a negative spike. This is intentional.

## Testing

Given a rendered HTML for a single-run report:

> When the renderer runs,
> then the HTML contains the five chart container IDs
> (`chart-cpu`, `chart-mem`, `chart-threads`, `chart-fds`,
> `chart-io`), in that order.

Given a multi-run rendered HTML:

> When the renderer runs,
> then the summary table contains one row per run with a swatch cell
> matching the per-run palette colour.

Given two consecutive samples with cumulative `io_read_bytes` of `0`
then `1_048_576`, exactly `0.5` seconds apart:

> When the viewer computes the read rate for the second sample,
> then the rate equals `2_097_152` bytes per second (2 MiB/s).

## Notes

- Related: [`viewer-diff-mode`](viewer-diff-mode.md) (palette and
  multi-run overlay), [`viewer-chart-interaction`](viewer-chart-interaction.md)
  (collapse, cursor sync, legend stability),
  [`viewer-self-contained-html`](viewer-self-contained-html.md)
  (packaging).
