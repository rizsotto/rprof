// SPDX-License-Identifier: MIT

//! End-to-end tests for `rprof run`.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use rprof::schema::{Footer, Header, Record, Sample};

fn rprof_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rprof"))
}

#[derive(Debug, Default)]
struct ParsedReport {
    header: Option<Header>,
    samples: Vec<Sample>,
    footer: Option<Footer>,
    /// Lines that did not deserialise as a Record (corrupt / partial).
    skipped: usize,
}

fn parse_report(path: &Path) -> ParsedReport {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    parse_report_str(&text)
}

fn parse_report_str(text: &str) -> ParsedReport {
    let mut out = ParsedReport::default();
    for line in text.split_inclusive('\n') {
        let complete = line.ends_with('\n');
        let trimmed = line.trim_matches(|c: char| c == '\n' || c == '\r');
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Record>(trimmed) {
            Ok(Record::Header(h)) => out.header = Some(h),
            Ok(Record::Sample(s)) => out.samples.push(s),
            Ok(Record::Footer(f)) => out.footer = Some(f),
            Err(_) => {
                if complete {
                    out.skipped += 1;
                }
                // Else: trailing partial line; per the schema, ignore.
            }
        }
    }
    out
}

// Requirements: capture-cli-contract, schema-v1, capture-streaming-write
#[test]
fn run_sleep_produces_valid_report() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sleep",
            "0.3",
        ])
        .status()
        .expect("rprof should run");
    assert_eq!(status.code(), Some(0), "rprof should mirror sleep's exit 0");

    let r = parse_report(&out);
    let header = r.header.expect("header present");
    let footer = r.footer.expect("footer present");
    assert_eq!(header.schema, 1);
    assert_eq!(header.tool.name, "rprof");
    assert_eq!(header.run.backend, "proc");
    assert_eq!(header.run.command, vec!["sleep".to_string(), "0.3".into()]);
    assert_eq!(footer.exit_code, Some(0));
    assert!(footer.signal.is_none());
    assert!(
        footer.wall_duration_ms >= 250,
        "wall_duration_ms too small: {}",
        footer.wall_duration_ms
    );
    assert!(!r.samples.is_empty(), "should produce samples");
    // Samples must be in ascending t_ms order.
    for w in r.samples.windows(2) {
        assert!(
            w[0].t_ms <= w[1].t_ms,
            "samples not in ascending t_ms order"
        );
    }
}

// Requirements: capture-exit-code-propagation
#[test]
fn run_propagates_nonzero_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "exit 42",
        ])
        .status()
        .expect("rprof should run");
    assert_eq!(status.code(), Some(42), "exit code should pass through");

    let r = parse_report(&out);
    let footer = r.footer.expect("footer present");
    assert_eq!(footer.exit_code, Some(42));
}

// Requirements: capture-cli-contract
#[test]
fn run_records_command_args() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "echo hi >/dev/null",
        ])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    let r = parse_report(&out);
    let h = r.header.expect("header present");
    assert_eq!(h.run.command, vec!["sh", "-c", "echo hi >/dev/null"]);
}

// Requirements: capture-proc-backend, capture-cpu-pct
#[test]
fn run_burning_cpu_records_nonzero_cpu_time() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sh",
            "-c",
            "timeout 0.3 yes >/dev/null || true",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let r = parse_report(&out);
    let f = r.footer.expect("footer present");
    let total_cpu = f.user_cpu_ms + f.system_cpu_ms;
    assert!(
        total_cpu > 0,
        "cpu busy workload should produce non-zero rusage cpu time, got {total_cpu}ms"
    );
}

// Requirements: capture-output-path
#[test]
fn run_writes_auto_output_path_when_no_dash_o() {
    let tmp = tempfile::tempdir().unwrap();
    let status = Command::new(rprof_bin())
        .current_dir(tmp.path())
        .args(["run", "--", "true"])
        .status()
        .unwrap();
    assert!(status.success());
    let rprof_dir = tmp.path().join(".rprof");
    assert!(rprof_dir.is_dir(), "expected .rprof/ to be created");
    let entries: Vec<_> = std::fs::read_dir(&rprof_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "exactly one report should be written");
    let path = entries[0].path();
    assert_eq!(path.extension().and_then(|s| s.to_str()), Some("jsonl"));
    let r = parse_report(&path);
    assert_eq!(r.footer.expect("footer present").exit_code, Some(0));
}

// Requirements: capture-signal-forwarding, capture-streaming-write
#[test]
fn run_forwards_sigint_and_still_writes_report() {
    use std::thread::sleep;

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let mut child = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sleep",
            "30",
        ])
        .spawn()
        .expect("spawn rprof");
    sleep(Duration::from_millis(250));
    // SAFETY: child.id() is a valid PID and SIGINT is well-defined.
    let rc = unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    assert_eq!(
        rc,
        0,
        "kill should succeed: errno={}",
        std::io::Error::last_os_error()
    );

    let status = child.wait().expect("wait on rprof");
    assert!(out.exists(), "report must be written even after SIGINT");

    let r = parse_report(&out);
    let f = r.footer.expect("footer present after a catchable signal");
    assert!(
        f.wall_duration_ms < 5000,
        "should have exited quickly after SIGINT, wall={}ms",
        f.wall_duration_ms
    );
    assert!(
        !status.success(),
        "rprof should propagate non-success on SIGINT"
    );
    assert!(
        f.exit_code.is_some() || f.signal.is_some(),
        "footer should reflect how the child died"
    );
}

// Requirements: capture-peak-rss-accuracy
#[test]
fn run_peak_rss_matches_known_allocation_within_5pct() {
    let mb: u64 = 64;
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("alloc.jsonl");
    let bin = rprof_bin();
    let status = Command::new(&bin)
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "20ms",
            "--",
        ])
        .arg(&bin)
        .args(["__alloc-fixture", &mb.to_string(), "0.6"])
        .status()
        .expect("rprof should run");
    assert!(status.success(), "fixture run should succeed");

    let r = parse_report(&out);
    // Peak RSS is reader-derived: max across all sample rss_bytes.
    let actual = r.samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);
    let expected_bytes = mb * 1024 * 1024;
    let process_overhead = 16 * 1024 * 1024;
    let lower = (expected_bytes as f64 * 0.95) as u64;
    let upper = ((expected_bytes as f64 * 1.05) as u64) + process_overhead;
    assert!(
        actual >= lower && actual <= upper,
        "peak rss {} not within ±5% of {} MiB allocation (allowed: {}..{})",
        actual,
        mb,
        lower,
        upper,
    );
}

// Requirements: capture-cli-contract
#[test]
fn run_help_mentions_double_dash_separator() {
    let out = Command::new(rprof_bin())
        .args(["run", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(
        s.contains("--") && s.to_lowercase().contains("command"),
        "help should describe `--` and the command argument: {s}"
    );
}

// Requirements: capture-streaming-write, schema-v1
#[test]
fn run_emits_records_in_order_header_samples_footer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sleep",
            "0.25",
        ])
        .status()
        .expect("rprof should run");
    assert!(status.success());

    let text = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        lines.len() >= 3,
        "expected header + ≥1 sample + footer, got {} lines",
        lines.len()
    );
    let kinds: Vec<&str> = lines
        .iter()
        .map(|l| {
            let r: Record = serde_json::from_str(l).expect("each line parses");
            match r {
                Record::Header(_) => "header",
                Record::Sample(_) => "sample",
                Record::Footer(_) => "footer",
            }
        })
        .collect();
    assert_eq!(kinds[0], "header", "first record must be header");
    assert_eq!(
        kinds[kinds.len() - 1],
        "footer",
        "last record must be footer"
    );
    assert!(
        kinds[1..kinds.len() - 1].iter().all(|k| *k == "sample"),
        "middle records must all be sample, got {kinds:?}"
    );
}

// Requirements: capture-streaming-write
#[test]
fn run_writes_samples_incrementally_during_long_run() {
    // The streaming-write contract: a live reader sees the sample count
    // grow as the run progresses. Spawn rprof against `sleep 1.5` at a
    // 100ms interval; poll the file every ~250ms and assert the parsed
    // sample count grows monotonically (and is non-zero before the run
    // ends).
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let mut child = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "100ms",
            "--",
            "sleep",
            "1.5",
        ])
        .spawn()
        .expect("spawn rprof");

    let started = Instant::now();
    let mut counts: Vec<usize> = Vec::new();
    while started.elapsed() < Duration::from_millis(900) {
        std::thread::sleep(Duration::from_millis(250));
        if let Ok(text) = std::fs::read_to_string(&out) {
            let r = parse_report_str(&text);
            counts.push(r.samples.len());
        }
    }
    // Make sure we drain the child.
    let status = child.wait().expect("wait rprof");
    assert!(status.success());

    // At least one observation must have non-zero samples, and the
    // sequence must be monotonically non-decreasing — samples can only
    // grow, never shrink.
    let max_during_run = counts.iter().copied().max().unwrap_or(0);
    assert!(
        max_during_run > 0,
        "expected to see at least one sample mid-run, observations: {counts:?}"
    );
    for w in counts.windows(2) {
        assert!(
            w[0] <= w[1],
            "sample count must not decrease during a run, observations: {counts:?}"
        );
    }
}

// Requirements: capture-streaming-write
#[test]
fn run_killed_with_sigkill_leaves_header_and_samples_no_footer() {
    use std::thread::sleep;

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.jsonl");
    let mut child = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sleep",
            "30",
        ])
        .spawn()
        .expect("spawn rprof");
    // Give rprof time to write the header and at least one sample.
    sleep(Duration::from_millis(300));
    // SIGKILL — uncatchable. No footer can be written.
    let rc = unsafe { libc::kill(child.id() as i32, libc::SIGKILL) };
    assert_eq!(rc, 0, "kill should succeed");
    let _ = child.wait();

    let r = parse_report(&out);
    assert!(r.header.is_some(), "header must be on disk after SIGKILL");
    assert!(
        !r.samples.is_empty(),
        "at least one sample must be on disk after SIGKILL"
    );
    assert!(
        r.footer.is_none(),
        "no footer should be present after SIGKILL"
    );
}
