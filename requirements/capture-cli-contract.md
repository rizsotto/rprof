---
title: rprof run command contract
status: implemented
---

## Intent

Users want `rprof run -- <cmd>` to behave exactly like running `<cmd>`
directly, with the only difference being that a JSON resource-usage report
is written on the side. The child's stdout/stderr must appear unchanged in
the user's terminal, its stdin must remain interactive, and its exit status
must reach whatever invoked `rprof`. If that contract slips, `rprof run`
stops being safe to drop into a shell pipeline or CI step.

## Acceptance criteria

- `rprof run -- <program> [args...]` spawns `<program>` exactly once.
- The `--` separator is required. Everything after `--` is forwarded
  verbatim to the child; nothing after `--` is parsed by rprof.
- The child inherits the parent's stdin, stdout, and stderr.
- The child's working directory is the same as `rprof`'s working directory
  at invocation time (no `chdir`).
- The child inherits rprof's full environment.
- Running with no command (`rprof run` with nothing after `--`) fails with
  a non-zero exit and a clear error message.
- `rprof run --help` documents the `--` separator and lists every flag.

## Non-functional constraints

- The polling overhead must not visibly change the child's runtime. At the
  100 ms default interval, the sampler thread should consume well under
  1 % of one core.
- `rprof` must not perform any blocking I/O in between spawning the child
  and starting the sampler thread; sampling latency from t=0 should be
  bounded by the configured interval.

## Implementation details

- `clap`'s `trailing_var_arg = true` plus `num_args = 1..` on the
  `command` field captures the post-`--` arguments without further
  parsing.
- `std::process::Command` with `Stdio::inherit()` on all three streams
  spawns the child.
- The current working directory is implicit: `Command` inherits it.
- See `src/runner.rs`.

## Known limitations

- No `--` enforcement at parse time when the first positional looks like
  a flag: `rprof run --interval 50ms cargo build` (without `--`) will
  treat `cargo` as the program and `build` as its arg, but a flag like
  `cargo --release` will be mis-parsed as an rprof flag. Always use `--`.
- The environment is not filtered. If the child must not see a secret in
  the environment, the caller has to scrub it before invoking rprof.

## Testing

Given a command that exits successfully:

> When the user runs `rprof run -- sleep 0.3`,
> then `rprof` exits with status 0
> and a JSON report is written to the path given by `-o` (or to
> `./.rprof/<timestamp>.json` if `-o` is omitted).

Given a command line with no program after `--`:

> When the user runs `rprof run --`,
> then `rprof` exits with a non-zero status and prints an error
> mentioning that no command was provided.

Given a command that writes to stdout and stderr:

> When the user runs `rprof run -- echo hello`,
> then "hello" appears on `rprof`'s stdout, not on its stderr,
> and the JSON report is still written successfully.

Given `rprof run --help`:

> When the user reads the help text,
> then the `--` separator is documented and every flag is listed with a
> short description.

## Notes

- The decision to require `--` (rather than letting rprof's flag parser
  stop at the first positional) avoids the classic "did rprof eat my
  flag?" foot-gun that `time(1)` and similar wrappers suffer from.
