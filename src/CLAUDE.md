# CLAUDE.md — `src/` guide

How to write code in this crate. *What* each module is for lives in its
own `//!` doc comment — read the file. The behavioural *contract* a
module implements lives in [`../docs/requirements/`](../docs/requirements/);
the *why* behind a non-obvious choice lives in
[`../docs/rationale/`](../docs/rationale/). This guide is the house
style: the narrow slice of Rust we use and the conventions that keep the
crate small and testable.

`rprof` is one binary with a `run` and a `view` subcommand, built as a
library (`pub` modules in `lib.rs`) so integration tests in `tests/` can
link against its types. A new file opens with a `//!` saying what the
module is for, then gets declared in `lib.rs`.

## Conventions

The cross-cutting Rust house style lives in
[`../.claude/rules/rust.md`](../.claude/rules/rust.md) and is applied
automatically when you edit any `.rs` file. The sections below add only
what is specific to *this* crate.

### Modules

- Keep the module tree flat: top-level files under `src/`, declared in
  `lib.rs`. No nested module directories — the crate is small enough
  that one level navigates faster than a hierarchy.
- A private inline submodule is justified only to hide a seam, the way
  `proc_backend` inside `sampler.rs` encapsulates the Linux-specific
  reads. Don't introduce one just to "organise"; reach for a new
  top-level module instead, and only when it owns a distinct
  responsibility you can state in one `//!` line.
- Prefer extending an existing module over adding one.

### Traits and abstraction

- Introduce a trait only for a real polymorphism seam with a second
  implementation actually in sight. The one trait today — `Sampler` — is
  the capture-backend seam (Linux `ProcSampler` now, macOS planned). No
  speculative traits, no generics for their own sake, no abstraction
  added "in case".
- `Sampler::sample()` returns `Ok(None)` when the target is gone; that
  is the graceful-stop protocol the run loop depends on. A new backend
  must honour it — do not turn target-gone into an `Err`.

### Concurrency

- The crate is almost entirely single-threaded. The *only* concurrency
  lives in `runner.rs`: one background sampler thread (spawned at start,
  joined after the child exits) and an async-signal-safe signal
  forwarder. Keep it that way.
- No async runtime (no `tokio`), no thread pools, no `Arc<Mutex<…>>`
  sharing. The sampler thread communicates over a `std::sync::mpsc`
  channel and sleeps on `recv_timeout(interval)`; the signal handler
  touches only a `static AtomicI32` and `libc::kill` — the only work
  async-signal-safety permits. Don't widen either surface.

### Defensive `/proc` reads

- Every `/proc/<pid>` read must tolerate `ENOENT` mid-sample: a process
  can vanish between `readdir` and `open`. That is normal, not an error.

### Computation lives in the reader

- The writer records raw cumulative counters; the viewer derives the
  rest (CPU %, IO rate, peak RSS) on load. Put new derivations on the
  read side so the on-disk schema stays minimal and can evolve without a
  version bump.

### Purity for testability

- Keep render/derive functions side-effect-free so a test can call them
  directly: `render_html()` is `pub` and does no filesystem writes or
  `Command` execution. Push IO to the edges (`runner.rs`, the `view`
  dispatch in `cli.rs`).

### Tests

How tests are written and where they live — in-file unit tests for pure
functions, integration tests under `tests/`, the `examples/` workload
fixtures, the `// Requirements: <id>` tags — is the test house style in
[`../.claude/rules/testing.md`](../.claude/rules/testing.md) and
[`../tests/CLAUDE.md`](../tests/CLAUDE.md).

## Contracts these modules implement

This guide is house style, not behaviour. The user-visible contracts —
the mandatory `--` separator and exit-code mirroring, the
signal/report-on-exit guarantee, the frozen schema and its
forward-compatibility rules, the self-contained-HTML output and the
`</` → `<\/` escaping — live in
[`../docs/requirements/`](../docs/requirements/), pinned by the tagged
tests. Change behaviour there (and in its test) first; do not restate or
fork those contracts here, where they would drift.
