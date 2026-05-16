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

## Capture

```
rprof run -o build-report.json -- cargo build --release
rprof run --interval 50ms -o slow.json -- ./slow-script.sh
```

The `--` separator is mandatory: everything after it is forwarded verbatim to
the child. The child inherits stdin/stdout/stderr and `rprof` mirrors its exit
code.

## View

```
rprof view build-report.json
rprof view --label before:before.json --label after:after.json
rprof view --no-open -o report.html run.json
```

The viewer writes a self-contained HTML file (data + JS + CSS inlined) and
opens it via `xdg-open` / `open`. With `--no-open` the HTML is written to the
given path or stdout.

## Status

Phase 1: `/proc`-based capture on Linux. macOS and cgroup v2 are planned for
later phases. See [`idea.md`](idea.md) for the full design and roadmap.

## License

MIT
