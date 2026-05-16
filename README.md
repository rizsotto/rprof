# rprof — Process Resource Profiler

`rprof` captures CPU and memory usage of a child process over time, writes the
time series to JSON, and renders interactive charts in a browser.

The tool is split into two subcommands so capture can run anywhere — in CI, on
a remote server, inside a container — while visualization stays local to
wherever a browser is available.

## Install

```
cargo install --path .
```

The release binary is a single static file (≈1 MB) with no runtime
dependencies. Drop it into a container image or CI step and it works.

## Capture

```
rprof run -o build-report.json -- cargo build --release
rprof run --interval 50ms -o slow.json -- ./slow-script.sh
rprof run --include-children -o tree.json -- make ci
```

The `--` separator is mandatory: everything after it is forwarded verbatim to
the child. The child inherits stdin/stdout/stderr and `rprof` mirrors its exit
code, so `rprof run` is drop-in compatible with shell pipelines. SIGINT,
SIGTERM and SIGHUP delivered to `rprof` are forwarded to the child, and the
JSON report is always written — Ctrl-C in CI never drops the data.

Without `-o`, the report lands under `./.rprof/<timestamp>.json`.

### Metrics captured per sample

- CPU% split into user and system (delta of `/proc/<pid>/stat` ticks)
- RSS and VSZ in bytes
- Thread count
- Open file descriptors
- IO bytes read and written (from `/proc/<pid>/io`)

The summary header also records peak RSS, total user/system CPU time (from
`getrusage(RUSAGE_CHILDREN)`), exit code or signal, command line, environment
fingerprint, host metadata, and the sample backend used.

## View

```
rprof view build-report.json
rprof view run-a.json run-b.json
rprof view --label before:before.json --label after:after.json
rprof view --no-open -o report.html run.json
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
