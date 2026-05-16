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

#[test]
fn view_no_open_writes_html_to_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let report = capture_report(tmp.path(), "r.json", "0.2");
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

#[test]
fn view_no_open_with_output_writes_file() {
    let tmp = tempfile::tempdir().unwrap();
    let report = capture_report(tmp.path(), "r.json", "0.2");
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

#[test]
fn view_overlays_two_reports_with_labels() {
    let tmp = tempfile::tempdir().unwrap();
    let a = capture_report(tmp.path(), "before.json", "0.15");
    let b = capture_report(tmp.path(), "after.json", "0.25");
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

#[test]
fn view_uses_filename_as_default_label() {
    let tmp = tempfile::tempdir().unwrap();
    let r = capture_report(tmp.path(), "my-build.json", "0.1");
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

#[test]
fn view_rejects_unknown_schema_version() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = tmp.path().join("bad.json");
    std::fs::write(
        &bad,
        r#"{
            "schema_version": 999,
            "tool": {"name":"rprof","version":"x"},
            "run":{"command":["x"],"cwd":"/","env_fingerprint":"00","start_time":"2026-01-01T00:00:00Z","wall_duration_ms":0,"exit_code":0,"signal":null,"backend":"proc","sample_interval_ms":100,"include_children":false},
            "host":{"hostname":"h","kernel":"x","cpu_count":1,"total_memory_bytes":0},
            "summary":{"peak_rss_bytes":0,"user_cpu_ms":0,"system_cpu_ms":0,"sample_count":0},
            "samples":[]
        }"#,
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
        stderr.contains("schema_version"),
        "error should mention schema version, got: {stderr}"
    );
}
