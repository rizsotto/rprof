---
title: Peak RSS accuracy
status: implemented
---

## Intent

A user who allocates a known buffer in their workload expects rprof's
reported peak RSS to match what they actually allocated. If the number is
off by an order of magnitude, the tool is useless for capacity planning
and regression detection.

## Acceptance criteria

- For a workload that allocates a known buffer of size `B` bytes and
  holds it alive for at least one sample interval, the reported
  `summary.peak_rss_bytes` lies within ±5 % of `B`, plus a small
  additive allowance for the workload's own runtime footprint.
- The reported peak is the maximum across all collected per-sample
  `rss_bytes`, not a rolling average.
- `rss_bytes` is in bytes (not pages, not KiB), computed as
  `/proc/<pid>/stat` field 24 (rss in pages) multiplied by
  `sysconf(_SC_PAGESIZE)`.

## Non-functional constraints

- The accuracy must hold across debug and release builds of the
  workload. The acceptance test pins this with the hidden
  `__alloc-fixture` subcommand of rprof itself.
- The accuracy must hold for allocations from 1 MiB up to system RAM.
  Smaller allocations are noisier because the workload's baseline RSS
  dominates.

## Implementation details

- `ProcStat::rss_pages` is parsed from field 24 of `/proc/<pid>/stat`.
- `ProcSampler::sample()` multiplies by the page size obtained from
  `sysconf(_SC_PAGESIZE)` at sampler construction time, defaulting to
  4096 if the syscall returns a non-positive value.
- The runner takes `max(samples.rss_bytes)` at finalisation time and
  stores it in `summary.peak_rss_bytes`.

## Known limitations

- Allocations made and freed *between* samples are missed entirely.
  This is fundamental to the polling approach.
- RSS counts shared memory mappings (e.g. glibc, libstdc++) against
  the process that maps them. For a single-process workload this
  matches what `ps`-style tools report.
- The kernel's `MMU_PAGE_SIZE` is assumed equal to the `sysconf`
  reported page size. This is true for x86_64 and aarch64 today.

## Testing

Given a workload that allocates 64 MiB of dirtied heap and sleeps:

> When `rprof run -- rprof __alloc-fixture 64 0.6` runs,
> then `summary.peak_rss_bytes` is between 95 % of 64 MiB and 105 % of
> 64 MiB plus a 16 MiB allowance for rprof's own footprint.

> When the same fixture is invoked with `vec![0u8; n]` only (no page
> dirtying), the peak RSS is *not* 64 MiB — Linux maps zeroed
> allocations to the shared zero page and only dirty writes create
> real RSS. This is documented in the fixture so future contributors
> know why the dirtying loop exists.

## Notes

- The fixture lives behind a hidden `__alloc-fixture` subcommand on
  rprof itself, so the integration test does not depend on Python,
  `dd`, or any other host tool.
- Observed accuracy on Fedora 44 is ~3.5 % over (66.3 MiB measured for
  a 64 MiB request in release builds).
