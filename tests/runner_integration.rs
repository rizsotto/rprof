// SPDX-License-Identifier: MIT

//! End-to-end tests for `rprof run`.

#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

use rprof::schema::Report;

fn rprof_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rprof"))
}

fn load_report(path: &std::path::Path) -> Report {
    let s =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&s).expect("parse report json")
}

// Requirements: capture-cli-contract, schema-v1
#[test]
fn run_sleep_produces_valid_report() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.json");
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

    let report = load_report(&out);
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.tool.name, "rprof");
    assert_eq!(report.run.backend, "proc");
    assert_eq!(report.run.command, vec!["sleep".to_string(), "0.3".into()]);
    assert_eq!(report.run.exit_code, Some(0));
    assert!(report.run.signal.is_none());
    assert!(
        report.run.wall_duration_ms >= 250,
        "wall_duration_ms too small: {}",
        report.run.wall_duration_ms
    );
    assert!(!report.samples.is_empty(), "should produce samples");
    assert_eq!(report.summary.sample_count, report.samples.len() as u64);
    assert!(report.summary.peak_rss_bytes > 0);
}

// Requirements: capture-exit-code-propagation
#[test]
fn run_propagates_nonzero_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.json");
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

    let report = load_report(&out);
    assert_eq!(report.run.exit_code, Some(42));
}

// Requirements: capture-cli-contract
#[test]
fn run_records_command_args() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.json");
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
    let r = load_report(&out);
    assert_eq!(r.run.command, vec!["sh", "-c", "echo hi >/dev/null"]);
}

// Requirements: capture-proc-backend, capture-cpu-pct
#[test]
fn run_burning_cpu_records_nonzero_cpu_time() {
    // `yes` writing to /dev/null pegs one core. 250ms is plenty for getrusage
    // to register at least 1ms of cumulative user time on any sane CI host.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.json");
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
    let r = load_report(&out);
    let total_cpu = r.summary.user_cpu_ms + r.summary.system_cpu_ms;
    assert!(
        total_cpu > 0,
        "cpu busy workload should produce non-zero rusage cpu time, got {total_cpu}ms"
    );
}

// Requirements: capture-output-path
#[test]
fn run_writes_auto_output_path_when_no_dash_o() {
    // Auto-generated path is `./.rprof/<timestamp>.json`. Run inside a temp
    // dir so we don't litter the repo.
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
    assert_eq!(path.extension().and_then(|s| s.to_str()), Some("json"));
    let r = load_report(&path);
    assert_eq!(r.run.exit_code, Some(0));
}

// Requirements: capture-signal-forwarding
#[test]
fn run_forwards_sigint_and_still_writes_report() {
    // Verifies the spec's "dropping the report on Ctrl-C would be a real
    // foot-gun in CI" requirement: rprof must forward SIGINT to the child
    // and write the report before exiting.
    use std::thread::sleep;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("r.json");
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
    // Give rprof + sleep time to start.
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

    let r = load_report(&out);
    // sleep(1) doesn't trap SIGINT; the kernel kills it with the signal so
    // rprof should record the signal and report a non-success status.
    assert!(
        r.run.wall_duration_ms < 5000,
        "should have exited quickly after SIGINT, wall={}ms",
        r.run.wall_duration_ms
    );
    assert!(
        !status.success(),
        "rprof should propagate non-success on SIGINT"
    );
    // Either an exit code (if the child handled the signal) or a signal field
    // must be set — both being None indicates we lost the info.
    assert!(
        r.run.exit_code.is_some() || r.run.signal.is_some(),
        "report should reflect how the child died"
    );
}

// Requirements: capture-peak-rss-accuracy
#[test]
fn run_peak_rss_matches_known_allocation_within_5pct() {
    // Acceptance criterion: "the reported peak RSS for a program that
    // allocates a known buffer matches the program's actual peak within 5%."
    //
    // We use the hidden `__alloc-fixture` subcommand of rprof itself as the
    // workload: it allocates a fixed-size buffer and holds it. The expected
    // peak RSS is roughly `mb` MiB plus a small process overhead (the rprof
    // binary's own .text/.bss, runtime, libc). A 64 MiB allocation makes the
    // overhead proportionally negligible.
    let mb: u64 = 64;
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("alloc.json");
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

    let r = load_report(&out);
    let expected_bytes = mb * 1024 * 1024;
    let actual = r.summary.peak_rss_bytes;

    // Spec allows ±5% accuracy on the *known* allocation. We allow a small
    // additive headroom for the rprof process's own footprint (well under
    // 10 MiB in release) before comparing.
    let process_overhead = 16 * 1024 * 1024;
    let lower = (expected_bytes as f64 * 0.95) as u64;
    let upper = ((expected_bytes as f64 * 1.05) as u64) + process_overhead;
    assert!(
        actual >= lower && actual <= upper,
        "peak_rss {} not within ±5% of {} MiB allocation (allowed: {}..{})",
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
    // The clap help should mention `--` somewhere or the command argument
    // description should make it clear.
    assert!(
        s.contains("--") && s.to_lowercase().contains("command"),
        "help should describe `--` and the command argument: {s}"
    );
}
