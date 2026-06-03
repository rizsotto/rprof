---
paths:
  - "**/*.rs"
---

# Rust house style

Conventions for every `.rs` file in the crate. The per-directory
`CLAUDE.md` files add only the rprof-specific structural and behavioural
constraints on top of these (the `Sampler` seam, the single sampler
thread, the `/proc` read discipline, computation-in-the-reader); they do
not restate what is here.

- **Edition / MSRV**: Rust 2021 idioms; MSRV `1.75` (see `Cargo.toml`).
  Don't reach for language or library features newer than 1.75.
- **File header**: every `.rs` file — under `src/`, `tests/`, and
  `examples/` — starts with `// SPDX-License-Identifier: MIT` as its very
  first line, before any `//!` module doc. rprof is MIT (see `LICENSE`).
- **Error handling**: `anyhow::Result` with `.context(...)` at the
  boundaries where a failure needs explaining. No bespoke error enums (no
  `thiserror`), and no error paths for states that cannot occur — `main.rs`
  turns any `Err` into exit 1.
- **Panicking macros**: `unwrap()` is for test code only. In production
  use `.expect("short reason")` only when a prior-stage invariant makes
  the `None`/`Err` structurally impossible, and name that invariant in the
  string (e.g. `.expect("non-empty command checked above")`). `panic!` /
  `unreachable!` are for unambiguous programmer bugs, with a one-line
  comment or message stating the violated invariant.
- **Modules**: extend an existing module before adding one; keep each
  module's public surface as small as the crate needs. rprof's concrete
  module layout — and when a new module is warranted — is in
  `src/CLAUDE.md`.
- **Abstraction**: introduce a trait only for a real polymorphism seam
  with a second implementation actually in sight. No speculative
  abstractions, no generics for their own sake. Prefer editing an existing
  file over adding a new one.
- **Dependencies**: resist adding crates. The single-static-binary,
  no-runtime-deps goal rides on a small dependency tree; a new dependency
  needs a real justification, not convenience.
- **Lint suppressions**: only `#[allow(...)]` when there is no better fix,
  always with a trailing `// reason` comment.
- **Comments** explain *why*, not *what*; default to none unless a subtle
  invariant would otherwise need re-deriving.
