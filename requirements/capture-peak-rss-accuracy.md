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
  holds it alive for at least one sample interval, the peak RSS the
  viewer derives from the captured `sample.rss_bytes` lies within
  ±5 % of `B`, plus a small additive allowance for the workload's
  own runtime footprint.
- The peak is the maximum across all collected per-sample
  `rss_bytes`, not a rolling average. It is **not** persisted as a
  separate aggregate field; the on-disk schema is per-sample only and
  the viewer recomputes the maximum on load (see
  [`schema-v1`](schema-v1.md)).
- `rss_bytes` is in bytes (not pages, not KiB), computed as
  `/proc/<pid>/stat` field 24 (rss in pages) multiplied by
  `sysconf(_SC_PAGESIZE)`.

## Non-functional constraints

- The accuracy must hold across debug and release builds of the
  workload. The acceptance test pins this with the `alloc_fixture`
  Cargo example binary that lives under `examples/`.
- The accuracy must hold for allocations from 1 MiB up to system RAM.
  Smaller allocations are noisier because the workload's baseline RSS
  dominates.

## Implementation details

- `ProcStat::rss_pages` is parsed from field 24 of `/proc/<pid>/stat`.
- `ProcSampler::sample()` multiplies by the page size obtained from
  `sysconf(_SC_PAGESIZE)` at sampler construction time, defaulting to
  4096 if the syscall returns a non-positive value.
- The viewer (`build_view_report` in `src/viewer.rs`) takes
  `max(samples.rss_bytes)` on load; nothing on the capture path
  retains the value.

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

> When `rprof run -- target/debug/examples/alloc_fixture 64 0.6` runs,
> then the maximum `sample.rss_bytes` value across the report's
> samples is between 95 % of 64 MiB and 105 % of 64 MiB plus a
> 16 MiB allowance for rprof's own footprint.

> When the same fixture is invoked with `vec![0u8; n]` only (no page
> dirtying), the peak RSS is *not* 64 MiB — Linux maps zeroed
> allocations to the shared zero page and only dirty writes create
> real RSS. This is documented in the fixture so future contributors
> know why the dirtying loop exists.

## Notes

- The fixture lives in `examples/alloc_fixture.rs`, built on demand by
  the integration test's `alloc_fixture_bin()` helper. Keeping it as a
  Cargo example rather than a hidden subcommand of `rprof` itself means
  the production binary doesn't carry test scaffolding while the
  integration test still avoids depending on Python, `dd`, or any
  other host tool.
- Observed accuracy on Fedora 44 is ~3.5 % over (66.3 MiB measured for
  a 64 MiB request in release builds).
