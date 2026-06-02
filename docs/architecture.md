# rprof architecture

How the two subcommands move data, end to end. For the responsibilities
of each source module see [`../src/CLAUDE.md`](../src/CLAUDE.md); for the
on-disk record format see
[`requirements/schema-v1.md`](requirements/schema-v1.md).

## `rprof run` — capture

```
rprof run -- cmd
   |
   v
open report file                                  (.rprof/<ts>.jsonl by default)
   |
   v
spawn child (stdio inherited)  ──>  signal forwarder (SIGINT/SIGTERM/SIGHUP)
   |
   v
write `header` record
   |
   v
sampler thread polls /proc/<pid> every --interval ms
   |   each tick: append a `sample` record, flush
   v
child exits  ──>  wait(), getrusage(RUSAGE_CHILDREN)
   |
   v
append `footer` record, flush, close
```

The report is written as a stream: the header lands immediately, each
sample is flushed as it is taken, and the footer is appended on exit.
A `kill -9` therefore leaves a partial-but-usable file rather than
nothing, and `tail -f` works during a capture.

## `rprof view` — render

```
rprof view r1.jsonl r2.jsonl ...
   |
   v
parse line-by-line; verify header.schema; tolerate unknown / truncated rows
   |
   v
derive per-sample CPU% and peak RSS in-memory
   |
   v
inline data + uPlot JS/CSS + viewer JS/CSS into single HTML
   |
   v
--no-open + no -o   ──>  HTML to stdout
--no-open + -o P    ──>  HTML written to P
no --no-open        ──>  HTML to -o or temp file, then xdg-open / open
```

Per-sample CPU% and peak RSS are derived by the reader, not stored: the
schema records cumulative ticks and per-sample RSS, and the viewer
computes the rest on load. This keeps the on-disk format minimal and
lets the viewer evolve its derivations without a schema bump.
