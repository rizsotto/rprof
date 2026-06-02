# CLAUDE.md — `scripts/` guide

Developer helpers that are not part of the shipped binary. Nothing in this
directory is consumed by `cargo build`, integration tests, or release
artefacts; these are humans-and-agents-only conveniences.

## Files

| File | Purpose |
|---|---|
| `dogfood.sh` | Dogfood `rprof` by running it against a `cargo build` of its own source, then rendering the viewer. Default: build → capture release & debug profiles → render HTML. Subcommands: `build`, `capture`, `view`, `all`. |
| `check-requirements-coverage.sh` | Verify every `status: implemented` requirement has at least one test tagged with `Requirements: <id>`. Exits non-zero with the list of gaps. Wired into the CI lint job; runnable locally. |

## When to reach for these

- **Iterating on the viewer (`assets/*.js`, `assets/*.css`, `src/viewer.rs`).**
  Run `scripts/dogfood.sh view` — rebuilds `rprof` incrementally (reuses the
  dev `target/` cache, ~seconds) and re-renders `dogfood/report.html` from
  the cached JSON. No re-capture needed.
- **Iterating on capture (`src/runner.rs`, `src/sampler.rs`, `src/proc_parse.rs`).**
  Run `scripts/dogfood.sh capture` (or `all`) — recaptures both JSON
  profiles, which is slow (one full release LTO build plus a debug build,
  typically 1–3 minutes total).
- **Smoke-testing end-to-end after a non-trivial change.**
  Run `scripts/dogfood.sh` with no arguments.

Always prefer the script over hand-rolling the equivalent `cargo build && rprof run ...`
sequence: the script keeps the workload directory isolated from the dev
`target/` (so captures stay cold) and writes outputs to predictable paths
under `dogfood/` (so agents and humans both know where to look).

## Outputs

All under `dogfood/` at the project root (gitignored):

- `bin/rprof` — the release binary the script invokes.
- `workload-release/`, `workload-debug/` — staged source copies used as the
  capture target. Wiped and re-staged on every `capture` so the build is
  always cold.
- `release.jsonl`, `debug.jsonl` — captured profiles.
- `report.html` — the rendered, self-contained viewer page.

## Rules

- The script must remain non-interactive and never invoke `xdg-open`. The
  primary environment is a devcontainer with no browser; viewer.rs already
  has a graceful fallback, but the script bypasses the question entirely by
  always passing `--no-open`.
- Do not add browser-launching logic, watchers, or auto-reload here. If you
  need those, run them outside the script.
- Outputs land under `dogfood/` only. Do not write into `target/`, `.rprof/`,
  or anywhere else under the project root.
- Scripts use POSIX `sh`, not bash. Shebang is `#!/bin/sh` and the script
  must run unmodified under dash (Ubuntu's `/bin/sh`, and the shell CI
  uses). Concretely: no `[[ ... ]]` (use `[ ... ]` with `=`, not `==`),
  no arrays, no `set -o pipefail`, no `<<<` here-strings, no
  bash-only parameter expansions. `set -eu` is the standard prologue.
  `local` inside functions is fine — dash, ash, busybox sh, and mksh
  all support it, even though it is not in POSIX.
- No Python, Node, or cargo plugins.
