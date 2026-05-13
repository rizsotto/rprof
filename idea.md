# rprof — Process Resource Profiler

A small UNIX-style toolbox utility that captures CPU and memory usage of a child process over time, writes the time series to JSON, and provides a separate viewer that renders interactive charts in a browser.

The tool is intentionally split into two binaries (or two subcommands of one binary) so that capture can run anywhere — in CI, on a remote server, inside a container — while visualization stays local to wherever a browser is available.

---

## Goals

- Single static binary, no runtime dependencies. Drop it into a container or a CI image and it works.
- Capture is non-invasive: no instrumentation, no LD_PRELOAD, no ptrace. The target program is unaware it is being measured.
- Output is plain JSON with a versioned schema. Trivially scriptable, diffable, archivable.
- Visualization is self-contained HTML. No long-running server, no port conflicts, no Python or Node required to view a report.
- Diffing multiple runs is a first-class feature, not an afterthought.

## Non-goals (v1)

- Flamegraphs and stack sampling. These require a different data acquisition path (`perf`, `dtrace`, eBPF) and a different visualization. Leave room for a future subcommand but do not build it now.
- Distributed tracing, network IO breakdown, GPU metrics, per-syscall accounting.
- Cross-platform Windows support. Linux first, macOS best-effort.
- A long-running daemon or system-wide monitor. This tool measures one command invocation.

---

## Functional Specification

### Capture

The capture subcommand spawns a child process, polls its resource usage at a configurable interval, and writes a JSON report on exit.

**Metrics collected per sample:**

- Timestamp (monotonic offset from start, in milliseconds)
- CPU percentage, split into user and system
- Resident set size (RSS), in bytes
- Virtual memory size (VSZ), in bytes
- Number of threads
- Number of open file descriptors
- IO: bytes read and written (from `/proc/<pid>/io` on Linux)
- Optional: per-child-process breakdown when the target spawns subprocesses

**Final summary written to the report header:**

- Command line, working directory, environment fingerprint (hash, not full env)
- Start time (wall clock, ISO 8601) and total wall duration
- Exit code and signal (if killed)
- Peak RSS, total user CPU time, total system CPU time (from `wait4` / `getrusage(RUSAGE_CHILDREN)`)
- Sample interval, number of samples, sample backend used
- Host metadata: hostname, kernel version, CPU count, total memory

**Data acquisition backends, in preference order:**

1. **cgroup v2** when available and the user has permission to create a transient cgroup. Most accurate, automatically covers the full process tree, handles short-lived grandchildren correctly.
2. **`/proc/<pid>` polling** with optional recursive walk of children. Portable across Linux, no privileges needed.
3. **`libproc` / `proc_pidinfo`** on macOS. Best-effort, may not capture all children reliably.

The backend is auto-detected but overridable via flag. The chosen backend is recorded in the report header.

### Viewer

The viewer subcommand takes one or more JSON reports and produces a self-contained HTML file containing the data inlined as a JSON blob and a small JavaScript bundle for rendering. By default it writes the HTML to a temp path and opens it via `xdg-open` / `open`. With `--no-open` it writes to stdout or a path.

**Charts rendered:**

- CPU% over time (user/system stacked or overlaid)
- RSS over time
- VSZ over time (toggleable, off by default)
- Threads and open FDs (small secondary panels)
- IO read/write rate (derivative of cumulative bytes)

**Interactions:**

- Hover anywhere on a chart to see the precise value at that timestamp across all panels (shared crosshair)
- Click and drag to zoom into a time range; double-click to reset
- Toggle individual series on/off via a legend

**Diff mode:** when multiple reports are passed, each chart overlays the runs with distinct colors and a legend keyed by either filename or a `--label` flag per file. Wall durations may differ; the X axis stays in absolute milliseconds rather than normalizing, but a `--align` flag can normalize to percentage of wall time.

**Summary panel:** above the charts, a table comparing peak RSS, total CPU time, wall time, exit code, and command line across all loaded runs.

### JSON Schema (sketch — to be finalized before implementation)

```json
{
  "schema_version": 1,
  "tool": { "name": "rprof", "version": "0.1.0" },
  "run": {
    "command": ["./build.sh", "--release"],
    "cwd": "/home/user/project",
    "start_time": "2026-05-14T10:30:00Z",
    "wall_duration_ms": 48230,
    "exit_code": 0,
    "signal": null,
    "backend": "cgroup_v2",
    "sample_interval_ms": 100
  },
  "host": {
    "hostname": "ci-runner-3",
    "kernel": "Linux 6.8.0",
    "cpu_count": 16,
    "total_memory_bytes": 33554432000
  },
  "summary": {
    "peak_rss_bytes": 1843200000,
    "user_cpu_ms": 124300,
    "system_cpu_ms": 8200
  },
  "samples": [
    {
      "t_ms": 0,
      "cpu_user_pct": 12.3,
      "cpu_sys_pct": 2.1,
      "rss_bytes": 45000000,
      "vsz_bytes": 320000000,
      "threads": 4,
      "open_fds": 18,
      "io_read_bytes": 0,
      "io_write_bytes": 0
    }
  ]
}
```

---

## User Experience

### Capture

```
rprof run -o build-report.json -- cargo build --release
rprof run --interval 50ms -o slow.json -- ./slow-script.sh
rprof run --backend proc -o ci.json -- make test
```

The `--` separator is mandatory and signals the end of rprof's own flags. Everything after it is forwarded verbatim to the child, including its flags. The child inherits stdin/stdout/stderr by default so the user sees its output exactly as if they had run it directly.

On exit, rprof prints a single-line summary to stderr (wall time, peak RSS, CPU time, exit code) and writes the JSON to the path given with `-o`. If `-o` is omitted, a path is auto-generated under `./.rprof/` with a timestamped filename. Exit code mirrors the child's exit code, so `rprof run` is drop-in compatible with shell pipelines and CI step exit handling.

### View

```
rprof view build-report.json
rprof view run-a.json run-b.json run-c.json
rprof view --label "before:run-a.json" --label "after:run-b.json"
rprof view --no-open -o report.html run.json
```

The default behavior — open a browser to a self-contained HTML file — is what most users want. The `--no-open` flag with an output path produces an artifact that can be attached to a bug report or pull request.

### Typical workflows

**Profile a CI build:**
```
rprof run -o ci-build.json -- make ci
# upload ci-build.json as a build artifact
```

**Compare two implementations:**
```
rprof run -o before.json -- ./oldscript.sh
rprof run -o after.json  -- ./newscript.sh
rprof view --label before:before.json --label after:after.json
```

**One-off investigation:**
```
rprof run -- ./suspicious-program
rprof view .rprof/2026-05-14T103000.json
```

---

## Delivery Plan

### Phase 0 — Clarifications before coding starts

These questions need answers (or explicit deferrals) before implementation. Each one would otherwise force a rewrite if decided late.

1. **Single binary or two binaries?** `rprof run` + `rprof view` as subcommands, or `rprof` and `rprof-view` as separate executables? Single binary with subcommands is the working assumption; confirm.
2. **Minimum supported Rust version (MSRV)** and target platforms. Working assumption: latest stable Rust, Linux x86_64 + aarch64, macOS aarch64 best-effort.
3. **Cgroup v2 backend: include in v1 or defer?** It is the most accurate option but adds complexity (creating a transient cgroup may need elevated privileges or a delegated cgroup). Working assumption: implement `/proc` polling first, add cgroup v2 in v1.1.
4. **Process tree handling.** When the target spawns children, do we sum metrics across the tree by default, or report only the direct child? Sum by default is the working assumption; flag to disable.
5. **Sample interval default.** 100ms is a sensible default for builds and scripts. Sub-50ms costs visible CPU on the polling side. Confirm 100ms.
6. **JSON compactness.** Is the per-sample object above acceptable, or do we need a columnar layout (parallel arrays, one per metric) to keep file sizes down for long runs? Columnar is ~3x smaller for typical runs. Decide before committing the schema.
7. **Schema versioning policy.** Working assumption: integer `schema_version`, viewer rejects unknown major versions, additive fields do not bump the version.
8. **JS charting library.** uPlot is the recommended choice (small, fast, good for time series). Confirm or substitute Chart.js if hover/legend ergonomics are a priority over bundle size.
9. **HTML self-containment.** Inline JS+CSS+data into a single HTML file, or write a directory with assets? Inline is the working assumption — one file is much easier to share.
10. **Distribution.** Cargo install only, or also prebuilt release binaries on GitHub, Homebrew formula, AUR, Nix? At minimum, prebuilt static Linux binaries via `cargo dist` from day one.
11. **License.** MIT, Apache-2.0, or dual?
12. **Name.** `rprof` is a placeholder. Confirm or substitute before publishing.

### Phase 1 — Capture MVP

Deliverable: `rprof run` works on Linux with the `/proc` backend, writes a valid JSON report.

- Project skeleton, CI (build + test + clippy + rustfmt), license, README stub.
- CLI argument parsing with `clap`. Subcommands `run` and `view` stubbed out.
- Child process spawning with stdio inheritance. Signal forwarding (SIGINT, SIGTERM) from rprof to the child. Correct exit code propagation.
- `/proc/<pid>` polling loop on a separate thread with a configurable interval. Graceful handling of the child exiting mid-sample.
- Recursive walk of `/proc/<pid>/task/*/children` to capture the full process tree.
- Final summary via `wait4` / `getrusage(RUSAGE_CHILDREN)`.
- JSON serialization with `serde`. Schema frozen at v1.
- Unit tests for the `/proc` parsers (feed in fixture files). Integration test that runs a known workload (`sleep`, a small Python script that allocates N MB) and asserts the report shape.

### Phase 2 — Viewer MVP

Deliverable: `rprof view report.json` opens a browser with working charts.

- Embed the JS+CSS bundle as compile-time assets via `include_str!`.
- HTML template with the report JSON inlined into a `<script type="application/json">` tag.
- uPlot (or chosen library) wired up for CPU, RSS, threads, FDs panels.
- Shared crosshair hover across panels with formatted tooltips (bytes → MiB/GiB, ms → s).
- Summary table above the charts.
- `xdg-open` / `open` integration with `--no-open` escape hatch.

### Phase 3 — Diff and polish

Deliverable: multi-report viewing, macOS support, packaging.

- Multi-file `rprof view a.json b.json` with overlay rendering and per-series legend.
- `--label` flag for naming runs in the legend and summary table.
- `--align` flag to normalize the X axis to percentage of wall time.
- macOS backend via `libproc`. Document any metrics that are unavailable or approximate on macOS.
- Prebuilt binaries via `cargo dist`. Homebrew tap or formula. Document install paths.
- User-facing documentation: README with the four workflows above, a SCHEMA.md describing the JSON format, a CONTRIBUTING.md.

### Phase 4 — Optional, post-v1

- Cgroup v2 backend.
- Columnar JSON layout behind a flag, with viewer support for both shapes.
- Flamegraph subcommand (`rprof flame`) that wraps `perf record` on Linux and renders via the inferno crate. Kept strictly separate from the resource-usage data path.
- Live mode: `rprof run --serve` exposes a local HTTP endpoint that streams samples to a browser while the child is still running. Defer until there is real demand — it complicates the architecture significantly.

### Risks and open questions to revisit during implementation

- **Sampling skew.** `/proc/<pid>/stat` reports cumulative jiffies; CPU% must be computed as a delta over wall time between samples. If the OS scheduler delays the polling thread, the reported interval will be wrong. Use a monotonic clock and record the actual elapsed time between samples, not the requested interval.
- **Short-lived grandchildren.** A 100ms poll will miss processes that live for less than 100ms. This is fundamental to the polling approach. Document it. The cgroup backend does not have this limitation.
- **Signal handling correctness.** If rprof receives SIGINT, it must forward to the child and wait for the child's exit before writing the report. Dropping the report on Ctrl-C would be a real foot-gun in CI.
- **Time-zone and clock drift in long runs.** Use a monotonic clock for sample timestamps; only the run start time uses wall clock.

---

## Acceptance criteria for v1

- A user can run `rprof run -o r.json -- <any command>` and get a JSON report whose contents match the documented schema.
- The reported peak RSS for a program that allocates a known buffer matches the program's actual peak within 5%.
- A user can run `rprof view r.json` on a machine with no Rust toolchain and no Node installed and see interactive charts in their browser.
- `rprof view a.json b.json` overlays both runs on every chart with a working legend.
- The release tarball is a single static binary under 10 MB.
- `rprof --help` and every subcommand's `--help` are sufficient to use the tool without reading the README.
