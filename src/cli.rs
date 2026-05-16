// SPDX-License-Identifier: MIT

//! Command-line entry point. The actual subcommand implementations live in
//! [`crate::runner`] and [`crate::viewer`].

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "rprof",
    version,
    about = "Capture CPU/memory time series of a child process and render charts."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Spawn a command and capture its resource usage to a JSON report.
    Run(RunArgs),
    /// Render one or more JSON reports as a self-contained HTML file.
    View(ViewArgs),
    /// Hidden test fixture: allocate `mb` megabytes of resident memory, then
    /// sleep for `seconds`. Used by integration tests to validate the peak
    /// RSS accuracy acceptance criterion.
    #[command(name = "__alloc-fixture", hide = true)]
    AllocFixture {
        /// Megabytes of heap to allocate and dirty.
        mb: usize,
        /// Seconds to hold the allocation alive.
        seconds: f64,
    },
}

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    /// Output JSON path. Defaults to `./.rprof/<timestamp>.json`.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Sample interval. Accepts forms like `100ms`, `1s`, `50ms`. Default: 100ms.
    #[arg(long, value_parser = parse_duration, default_value = "100ms")]
    pub interval: Duration,

    /// Backend to use. `auto` picks the best available.
    #[arg(long, default_value = "auto", value_parser = ["auto", "proc"])]
    pub backend: String,

    /// Aggregate metrics across the whole process tree, not just the direct child.
    #[arg(long)]
    pub include_children: bool,

    /// Command to run. Everything after `--` is forwarded verbatim to the child.
    #[arg(trailing_var_arg = true, required = true, num_args = 1..)]
    pub command: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct ViewArgs {
    /// One or more JSON report files to render. Multiple files trigger diff mode.
    ///
    /// May be omitted if at least one `--label LABEL:PATH` is provided.
    pub reports: Vec<PathBuf>,

    /// Label a report. Format: `label:path`. May be repeated. Labels override
    /// the default filename-based label and may stand alone in place of a
    /// positional report path.
    #[arg(long = "label", value_parser = parse_label)]
    pub labels: Vec<(String, PathBuf)>,

    /// Write the HTML to this path instead of opening a browser.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Do not open a browser. With no `-o`, the HTML is written to stdout.
    #[arg(long)]
    pub no_open: bool,
}

/// Parse a duration string like `100ms` or `1s`.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let (num, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else {
        return Err(format!("expected suffix `ms` or `s`, got `{s}`"));
    };
    let n: u64 = num
        .parse()
        .map_err(|e| format!("invalid number `{num}`: {e}"))?;
    let d = match unit {
        "ms" => Duration::from_millis(n),
        "s" => Duration::from_secs(n),
        _ => unreachable!(),
    };
    if d.is_zero() {
        return Err("interval must be > 0".into());
    }
    Ok(d)
}

fn parse_label(s: &str) -> Result<(String, PathBuf), String> {
    let (label, path) = s
        .split_once(':')
        .ok_or_else(|| format!("expected `label:path`, got `{s}`"))?;
    if label.is_empty() {
        return Err("label cannot be empty".into());
    }
    Ok((label.to_string(), PathBuf::from(path)))
}

/// Entry point used from `main.rs`. Returns the exit code to propagate.
pub fn run() -> Result<u8> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run(args) => crate::runner::run(args).context("rprof run failed"),
        Cmd::View(args) => crate::viewer::run(args).context("rprof view failed"),
        Cmd::AllocFixture { mb, seconds } => alloc_fixture(mb, seconds),
    }
}

/// Allocate and dirty `mb` megabytes of heap, then hold it for `seconds`.
///
/// `vec![0u8; n]` is *not* enough on Linux: zeroed u8 allocations get backed
/// by the kernel's shared zero page (CoW), so RSS stays tiny. We touch one
/// byte per 4 KiB page with a non-zero value to force real page allocation.
fn alloc_fixture(mb: usize, seconds: f64) -> Result<u8> {
    let bytes = mb.saturating_mul(1024 * 1024);
    let mut buf: Vec<u8> = vec![0u8; bytes];
    const PAGE: usize = 4096;
    let mut i = 0;
    while i < bytes {
        buf[i] = 0xa5;
        i += PAGE;
    }
    std::hint::black_box(&buf);
    std::thread::sleep(Duration::from_secs_f64(seconds.max(0.0)));
    drop(buf);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_ms() {
        assert_eq!(parse_duration("100ms").unwrap(), Duration::from_millis(100));
        assert_eq!(parse_duration("1s").unwrap(), Duration::from_secs(1));
    }

    #[test]
    fn parse_duration_rejects_zero() {
        assert!(parse_duration("0ms").is_err());
    }

    #[test]
    fn parse_duration_rejects_bare_number() {
        assert!(parse_duration("100").is_err());
    }

    #[test]
    fn parse_label_splits_on_first_colon() {
        let (label, path) = parse_label("before:foo.json").unwrap();
        assert_eq!(label, "before");
        assert_eq!(path, PathBuf::from("foo.json"));
    }

    #[test]
    fn parse_label_requires_colon() {
        assert!(parse_label("noseparator").is_err());
    }

    #[test]
    fn parse_label_rejects_empty_label() {
        assert!(parse_label(":foo.json").is_err());
    }
}
