---
title: /proc polling backend (Linux)
status: implemented
---

## Intent

`rprof run` needs a sampling backend that works on any Linux host without
elevated privileges. `/proc/<pid>` polling is the obvious choice: every
modern Linux distro mounts `/proc`, no setuid is needed, no kernel modules
are involved. The backend produces per-sample readings of CPU ticks, RSS,
VSZ, threads, open FDs, and IO counters by reading and parsing several
files under `/proc/<pid>/`.

## Acceptance criteria

- Each tick produces a `RawSample` populated from:
  - `utime`, `stime`, `num_threads`, `vsize`, `rss` from
    `/proc/<pid>/stat` (fields 14, 15, 20, 23, 24).
  - `read_bytes`, `write_bytes` from `/proc/<pid>/io` (absent values
    default to 0).
  - The count of entries in `/proc/<pid>/fd/`.
- RSS is reported in bytes (`rss_pages * sysconf(_SC_PAGESIZE)`).
- When the target PID has vanished (`/proc/<pid>/stat` returns
  `ENOENT`), the sampler returns `Ok(None)` rather than an error. This
  is the polling loop's graceful stop signal.
- The backend identifies itself with the literal string `proc` in the
  report's `run.backend` field.
- The backend is gated by `#[cfg(target_os = "linux")]`. Other unixes
  do not get a default backend in v1.

## Non-functional constraints

- Polling at the 100 ms default interval must not consume more than a
  small single-digit percentage of one core.
- Every read tolerates `ENOENT` mid-sample. Processes can disappear
  between `readdir` and `open`; that is normal, not an error.

## Implementation details

- Parsers live in `src/proc_parse.rs` and operate on `&str` inputs so
  unit tests drive them with fixture data (real `/proc/<pid>/stat`
  contents pasted into string constants).
- `ProcStat::parse` handles the `comm` field's potential spaces and
  closing parens by splitting at the *last* `)` (per `man 5 proc`).
- `ProcIo::parse` ignores fields it does not care about and defaults
  missing fields to zero; older kernels and unprivileged processes
  may produce empty `io` files.
- `ProcSampler` in `src/sampler.rs` wraps the parsers. The struct is
  the canonical entry point; the internal `proc_backend` module is an
  implementation detail.

## Known limitations

- This backend samples one PID — the direct child of `rprof run`.
  Aggregating across the process tree is a non-goal (see the
  Non-goals section in [`../CLAUDE.md`](../CLAUDE.md)).
- `/proc/<pid>/io` is empty for processes the caller cannot ptrace.
  When that happens, IO counters stay at zero for the whole run.
- Linux only. macOS support via `libproc` / `proc_pidinfo` is planned
  separately; see
  [`capture-macos-backend`](capture-macos-backend.md).

## Testing

Given the parsers in isolation:

> When `ProcStat::parse` is given a realistic `stat` line, it returns
> the correct utime, stime, threads, vsize, and rss_pages.
> When the `comm` field contains a space or closing paren, the parser
> still finds the last `)` and parses correctly.

Given the sampler against the test process itself:

> When `ProcSampler::new(std::process::id()).sample()` runs,
> then it returns `Ok(Some(_))` with non-zero VSZ and RSS.
> When the same sampler is constructed with a PID that does not exist,
> `sample()` returns `Ok(None)`.

## Notes

- Future backends (e.g. `libproc` for macOS) will plug in via the same
  `Sampler` trait. The `run.backend` string is how reports declare
  which backend produced them.
- The /proc layout is stable Linux kernel API. We pin to fields by
  index (per `man 5 proc`) rather than by name; new fields added
  after field 24 do not affect us.
