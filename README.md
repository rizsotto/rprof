# rprof — Process Resource Profiler

`rprof` captures CPU and memory usage of a child process over time, streams
the time series to disk as JSON Lines, and renders interactive charts in a
browser.

The tool is split into two subcommands so capture can run anywhere — in CI, on
a remote server, inside a container — while visualization stays local to
wherever a browser is available.

Records are written as the run progresses, so a SIGKILL or host power loss
leaves a partial-but-usable file rather than nothing; `tail -f` works on a
capture in progress.

## Install

```
cargo install --path .
```

The release binary is a single static file (≈1 MB) with no runtime
dependencies. Drop it into a container image or CI step and it works.

## Capture

```
rprof run -o build-report.jsonl -- cargo build --release
rprof run --interval 50ms -o slow.jsonl -- ./slow-script.sh
```

`rprof` measures the single child process you give it. Aggregating across
the process tree (a wrapper shell, `make` driving sub-makes, etc.) is
intentionally out of scope — reach for cgroup-level tools
(`systemd-run --scope`, `cgexec`, `perf stat`) for that.

The `--` separator is mandatory: everything after it is forwarded verbatim to
the child. The child inherits stdin/stdout/stderr and `rprof` mirrors its exit
code, so `rprof run` is drop-in compatible with shell pipelines. SIGINT,
SIGTERM and SIGHUP delivered to `rprof` are forwarded to the child, and the
report is always written — Ctrl-C in CI never drops the data.

Without `-o`, the report lands under `./.rprof/<timestamp>.jsonl`.

### Metrics captured per sample

- CPU% split into user and system (delta of `/proc/<pid>/stat` ticks)
- RSS and VSZ in bytes
- Thread count
- Open file descriptors
- IO bytes read and written (from `/proc/<pid>/io`)

The report's `header` row records command line, environment fingerprint,
host metadata (including `clock_ticks_per_sec` so readers can convert CPU
ticks to seconds), and the sample backend used. The `footer` row, written
once the child has exited, records wall duration, total user/system CPU
time (from `getrusage(RUSAGE_CHILDREN)`), and the exit code or signal.
Peak RSS and per-sample CPU% are derived by the viewer at load time from
the per-sample rows.

## View

```
rprof view build-report.jsonl
rprof view run-a.jsonl run-b.jsonl
rprof view --label before:before.jsonl --label after:after.jsonl
rprof view --no-open -o report.html run.jsonl
```

The viewer writes a self-contained HTML file (uPlot bundle, CSS, and report
data inlined into one file, ≈60 KB plus the data) and opens it via `xdg-open`
on Linux or `open` on macOS. With `--no-open` the HTML is written to the
given `-o` path or to stdout — useful for attaching to a bug report or pull
request artifact.

Passing multiple reports overlays them on every chart with per-run colors and
a shared cursor across panels. `--label LABEL:PATH` overrides the default
filename-based label in the legend and summary table.

## License

MIT
