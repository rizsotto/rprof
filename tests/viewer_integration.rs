// SPDX-License-Identifier: MIT

//! End-to-end tests for `rprof view`.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

fn rprof_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rprof"))
}

/// Helper: capture a tiny report so the viewer has something to render.
fn capture_report(dir: &std::path::Path, name: &str, sleep_seconds: &str) -> PathBuf {
    let out = dir.join(name);
    let status = Command::new(rprof_bin())
        .args([
            "run",
            "-o",
            out.to_str().unwrap(),
            "--interval",
            "50ms",
            "--",
            "sleep",
            sleep_seconds,
        ])
        .status()
        .expect("rprof should run");
    assert!(status.success());
    out
}

// Requirements: viewer-self-contained-html
#[test]
fn view_no_open_writes_html_to_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let report = capture_report(tmp.path(), "r.jsonl", "0.2");
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open"])
        .arg(&report)
        .output()
        .expect("rprof view should run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).expect("HTML must be utf-8");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("uPlot"));
    assert!(html.contains("id=\"rprof-data\""));
    assert!(html.contains("chart-cpu"));
}

// Requirements: viewer-self-contained-html
#[test]
fn view_no_open_with_output_writes_file() {
    let tmp = tempfile::tempdir().unwrap();
    let report = capture_report(tmp.path(), "r.jsonl", "0.2");
    let html_path = tmp.path().join("report.html");
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open", "-o"])
        .arg(&html_path)
        .arg(&report)
        .output()
        .expect("rprof view should run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stdout.is_empty(),
        "should not print HTML to stdout when -o is given"
    );
    let html = std::fs::read_to_string(&html_path).expect("output file should exist");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("uPlot"));
}

// Requirements: viewer-diff-mode
#[test]
fn view_overlays_two_reports_with_labels() {
    let tmp = tempfile::tempdir().unwrap();
    let a = capture_report(tmp.path(), "before.jsonl", "0.15");
    let b = capture_report(tmp.path(), "after.jsonl", "0.25");
    let html_path = tmp.path().join("compare.html");
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open", "-o"])
        .arg(&html_path)
        .args(["--label"])
        .arg(format!("before:{}", a.display()))
        .args(["--label"])
        .arg(format!("after:{}", b.display()))
        .output()
        .expect("rprof view should run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("\"label\":\"before\""));
    assert!(html.contains("\"label\":\"after\""));
    assert!(html.contains("2 runs"));
}

// Requirements: viewer-diff-mode
#[test]
fn view_uses_filename_as_default_label() {
    let tmp = tempfile::tempdir().unwrap();
    let r = capture_report(tmp.path(), "my-build.jsonl", "0.1");
    let html_path = tmp.path().join("out.html");
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open", "-o"])
        .arg(&html_path)
        .arg(&r)
        .output()
        .unwrap();
    assert!(out.status.success());
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(
        html.contains("\"label\":\"my-build\""),
        "filename stem should be the default label"
    );
}

#[test]
fn view_rejects_no_inputs() {
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "view with no reports should fail");
}

// Requirements: schema-v1
#[test]
fn view_rejects_unknown_schema_version() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = tmp.path().join("bad.jsonl");
    std::fs::write(
        &bad,
        "{\"type\":\"header\",\"schema\":999,\"tool\":{\"name\":\"rprof\",\"version\":\"x\"},\"run\":{\"command\":[\"x\"],\"cwd\":\"/\",\"env_fingerprint\":\"00\",\"start_time\":\"2026-01-01T00:00:00Z\",\"backend\":\"proc\",\"sample_interval_ms\":100},\"host\":{\"hostname\":\"h\",\"kernel\":\"x\",\"cpu_count\":1,\"total_memory_bytes\":0,\"clock_ticks_per_sec\":100}}\n",
    )
    .unwrap();
    let out = Command::new(rprof_bin())
        .args(["view", "--no-open"])
        .arg(&bad)
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "should reject unknown schema version"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("schema"),
        "error should mention schema, got: {stderr}"
    );
    let bad_str = bad.display().to_string();
    assert!(
        stderr.contains(&bad_str),
        "error should mention the file path, got: {stderr}"
    );
}

// Requirements: schema-v1, capture-streaming-write
#[test]
fn view_renders_partial_file_without_footer() {
    // A file with header + samples but no footer (the SIGKILL case) must
    // still render. The viewer treats the run's end state as "unknown".
    use rprof::schema::{Header, Host, Record, Run, Sample, Tool, SCHEMA_VERSION};

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("partial.jsonl");
    let header = Record::Header(Header {
        schema: SCHEMA_VERSION,
        tool: Tool::current(),
        run: Run {
            command: vec!["sleep".into(), "5".into()],
            cwd: "/tmp".into(),
            env_fingerprint: "0".repeat(64),
            start_time: "2026-05-14T10:30:00Z".into(),
            backend: "proc".into(),
            sample_interval_ms: 100,
        },
        host: Host {
            hostname: "h".into(),
            kernel: "Linux".into(),
            cpu_count: 1,
            total_memory_bytes: 0,
            clock_ticks_per_sec: 100,
        },
    });
    let sample = Record::Sample(Sample {
        t_ms: 0,
        wall_ms: 0,
        utime_ticks: 0,
        stime_ticks: 0,
        rss_bytes: 1024,
        vsz_bytes: 2048,
        threads: 1,
        open_fds: 3,
        io_read_bytes: 0,
        io_write_bytes: 0,
    });
    let mut text = serde_json::to_string(&header).unwrap();
    text.push('\n');
    text.push_str(&serde_json::to_string(&sample).unwrap());
    text.push('\n');
    std::fs::write(&path, &text).unwrap();

    let out = Command::new(rprof_bin())
        .args(["view", "--no-open"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "partial file must render; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains("uPlot"));
    assert!(html.contains("id=\"rprof-data\""));
}

// Requirements: schema-v1
#[test]
fn view_tolerates_truncated_trailing_line() {
    // Mid-record truncation must render whatever well-formed lines
    // preceded it. The viewer treats the corrupt tail as "unknown end".
    use rprof::schema::{Header, Host, Record, Run, Sample, Tool, SCHEMA_VERSION};

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("torn.jsonl");
    let header = Record::Header(Header {
        schema: SCHEMA_VERSION,
        tool: Tool::current(),
        run: Run {
            command: vec!["echo".into()],
            cwd: "/tmp".into(),
            env_fingerprint: "0".repeat(64),
            start_time: "2026-05-14T10:30:00Z".into(),
            backend: "proc".into(),
            sample_interval_ms: 100,
        },
        host: Host {
            hostname: "h".into(),
            kernel: "Linux".into(),
            cpu_count: 1,
            total_memory_bytes: 0,
            clock_ticks_per_sec: 100,
        },
    });
    let sample = Record::Sample(Sample {
        t_ms: 0,
        wall_ms: 0,
        utime_ticks: 0,
        stime_ticks: 0,
        rss_bytes: 1024,
        vsz_bytes: 2048,
        threads: 1,
        open_fds: 3,
        io_read_bytes: 0,
        io_write_bytes: 0,
    });
    let mut text = serde_json::to_string(&header).unwrap();
    text.push('\n');
    text.push_str(&serde_json::to_string(&sample).unwrap());
    text.push('\n');
    // Append a truncated sample line: open brace, no closing brace, no
    // trailing newline.
    text.push_str("{\"type\":\"sample\",\"t_ms\":100");
    std::fs::write(&path, &text).unwrap();

    let out = Command::new(rprof_bin())
        .args(["view", "--no-open"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "truncated file must render; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
