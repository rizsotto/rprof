// SPDX-License-Identifier: MIT

//! `rprof run` implementation.
//!
//! Spawns the user's command with inherited stdio, polls `/proc/<pid>` on a
//! background thread at the configured interval, forwards SIGINT/SIGTERM to
//! the child, and writes a JSON report when the child exits.

use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use sha2::Digest;

use crate::cli::RunArgs;
use crate::sampler::RawSample;
use crate::schema::{Host, Report, Run, Sample, Summary, Tool, SCHEMA_VERSION};

/// Entry point used by the CLI dispatcher.
pub fn run(args: RunArgs) -> Result<u8> {
    if args.command.is_empty() {
        anyhow::bail!("no command provided after `--`");
    }
    if cfg!(not(target_os = "linux")) {
        anyhow::bail!("`rprof run` is only supported on Linux in this build");
    }
    #[cfg(target_os = "linux")]
    {
        run_linux(args)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        unreachable!()
    }
}

#[cfg(target_os = "linux")]
fn run_linux(args: RunArgs) -> Result<u8> {
    use crate::sampler::{ProcSampler, Sampler};

    let (program, child_args) = args
        .command
        .split_first()
        .expect("non-empty command checked above");

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let env_fp = env_fingerprint();
    let start_wall = chrono::Utc::now();
    let start_instant = Instant::now();

    let mut child = Command::new(program)
        .args(child_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn {program:?}"))?;
    let child_pid = child.id();

    install_signal_forwarder(child_pid as i32)?;

    let interval = args.interval;
    let include_children = args.include_children;
    let (stop_tx, stop_rx) = channel::<()>();

    let sampler_handle = thread::spawn(move || -> Result<Vec<(u64, u64, RawSample)>> {
        let mut sampler: Box<dyn Sampler> = Box::new(ProcSampler::new(child_pid, include_children));
        let mut out: Vec<(u64, u64, RawSample)> = Vec::new();
        loop {
            let t_ms = start_instant.elapsed().as_millis() as u64;
            let wall_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            match sampler.sample()? {
                Some(s) => out.push((t_ms, wall_ms, s)),
                None => break,
            }
            match stop_rx.recv_timeout(interval) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(out)
    });

    let status = child.wait().context("waiting for child to exit")?;
    let wall_duration = start_instant.elapsed();
    let _ = stop_tx.send(());
    clear_signal_forwarder();

    let raw_samples = sampler_handle
        .join()
        .map_err(|_| anyhow::anyhow!("sampler thread panicked"))??;

    let (user_cpu_ms, system_cpu_ms) = read_rusage_children();
    let samples = compute_samples(&raw_samples);
    let peak_rss = samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);
    let (exit_code, signal) = decode_status(&status);

    let report = Report {
        schema_version: SCHEMA_VERSION,
        tool: Tool::current(),
        run: Run {
            command: args.command.clone(),
            cwd,
            env_fingerprint: env_fp,
            start_time: start_wall.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            wall_duration_ms: wall_duration.as_millis() as u64,
            exit_code,
            signal,
            backend: "proc".to_string(),
            sample_interval_ms: interval.as_millis() as u64,
            include_children,
        },
        host: host_metadata(),
        summary: Summary {
            peak_rss_bytes: peak_rss,
            user_cpu_ms,
            system_cpu_ms,
            sample_count: samples.len() as u64,
        },
        samples,
    };

    let output_path = resolve_output_path(args.output.as_ref(), &start_wall)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating output directory {}", parent.display()))?;
        }
    }
    let json = serde_json::to_vec(&report).context("serializing report")?;
    std::fs::write(&output_path, &json)
        .with_context(|| format!("writing report to {}", output_path.display()))?;

    eprintln!(
        "rprof: wrote {} ({} samples, wall {}ms, peak_rss {}, cpu_user {}ms, cpu_sys {}ms, exit {})",
        output_path.display(),
        report.summary.sample_count,
        report.run.wall_duration_ms,
        format_bytes(report.summary.peak_rss_bytes),
        report.summary.user_cpu_ms,
        report.summary.system_cpu_ms,
        match (exit_code, signal) {
            (Some(c), _) => format!("{c}"),
            (None, Some(s)) => format!("signal {s}"),
            _ => "unknown".to_string(),
        },
    );

    Ok(exit_status_to_u8(exit_code, signal))
}

/// Convert raw cumulative samples into `Sample` records with CPU percentages.
///
/// The first sample has 0% CPU (no previous sample to delta against). All
/// subsequent samples compute `delta_ticks / clock_ticks_per_sec / dt_seconds`.
/// 100% means one fully-loaded core; a process pegging 4 cores reads as 400%.
fn compute_samples(raw: &[(u64, u64, RawSample)]) -> Vec<Sample> {
    let clk_tck = clock_ticks_per_second();
    let mut out = Vec::with_capacity(raw.len());
    for (i, (t_ms, wall_ms, s)) in raw.iter().enumerate() {
        let (cpu_user_pct, cpu_sys_pct) = if i == 0 {
            (0.0, 0.0)
        } else {
            let prev = &raw[i - 1];
            let dt_ms = (*t_ms).saturating_sub(prev.0).max(1);
            let dt_s = dt_ms as f64 / 1000.0;
            let du = s.utime_ticks.saturating_sub(prev.2.utime_ticks) as f64 / clk_tck;
            let ds = s.stime_ticks.saturating_sub(prev.2.stime_ticks) as f64 / clk_tck;
            ((du / dt_s) * 100.0, (ds / dt_s) * 100.0)
        };
        out.push(Sample {
            t_ms: *t_ms,
            wall_ms: *wall_ms,
            cpu_user_pct,
            cpu_sys_pct,
            rss_bytes: s.rss_bytes,
            vsz_bytes: s.vsz_bytes,
            threads: s.threads,
            open_fds: s.open_fds,
            io_read_bytes: s.io_read_bytes,
            io_write_bytes: s.io_write_bytes,
        });
    }
    out
}

#[cfg(target_os = "linux")]
fn clock_ticks_per_second() -> f64 {
    // SAFETY: `sysconf` is thread-safe.
    let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if v > 0 {
        v as f64
    } else {
        100.0
    }
}

#[cfg(not(target_os = "linux"))]
fn clock_ticks_per_second() -> f64 {
    100.0
}

fn env_fingerprint() -> String {
    let mut entries: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    entries.sort();
    let joined = entries.join("\n");
    let digest = sha2::Sha256::digest(joined.as_bytes());
    hex::encode(digest)
}

#[cfg(target_os = "linux")]
fn host_metadata() -> Host {
    let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let kernel = if osrelease.is_empty() {
        "Linux".to_string()
    } else {
        format!("Linux {osrelease}")
    };
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(0);
    Host {
        hostname,
        kernel,
        cpu_count,
        total_memory_bytes: read_meminfo_total().unwrap_or(0),
    }
}

#[cfg(not(target_os = "linux"))]
fn host_metadata() -> Host {
    Host {
        hostname: String::new(),
        kernel: String::new(),
        cpu_count: 0,
        total_memory_bytes: 0,
    }
}

#[cfg(target_os = "linux")]
fn read_meminfo_total() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

fn read_rusage_children() -> (u64, u64) {
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: `getrusage` writes into a valid `rusage` allocation.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, usage.as_mut_ptr()) };
    if rc != 0 {
        return (0, 0);
    }
    let usage = unsafe { usage.assume_init() };
    let user = timeval_to_ms(usage.ru_utime);
    let sys = timeval_to_ms(usage.ru_stime);
    (user, sys)
}

fn timeval_to_ms(tv: libc::timeval) -> u64 {
    let sec = tv.tv_sec.max(0) as u64;
    let usec = tv.tv_usec.max(0) as u64;
    sec.saturating_mul(1000).saturating_add(usec / 1000)
}

fn decode_status(status: &ExitStatus) -> (Option<i32>, Option<i32>) {
    use std::os::unix::process::ExitStatusExt;
    (status.code(), status.signal())
}

/// 128+signum is the POSIX convention; if the child exited normally, we just
/// pass the code through (lower 8 bits).
fn exit_status_to_u8(code: Option<i32>, signal: Option<i32>) -> u8 {
    if let Some(c) = code {
        // u8 truncation matches what the shell sees from $?.
        return (c & 0xff) as u8;
    }
    if let Some(s) = signal {
        return (128u32 + s as u32).min(255) as u8;
    }
    1
}

fn resolve_output_path(
    opt: Option<&PathBuf>,
    start: &chrono::DateTime<chrono::Utc>,
) -> Result<PathBuf> {
    match opt {
        Some(p) => Ok(p.clone()),
        None => {
            let dir = PathBuf::from(".rprof");
            let ts = start.format("%Y-%m-%dT%H%M%S").to_string();
            Ok(dir.join(format!("{ts}.json")))
        }
    }
}

fn format_bytes(b: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = b as f64;
    if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{b} B")
    }
}

// ---------------------------------------------------------------------------
// Signal forwarding
// ---------------------------------------------------------------------------

static CHILD_PID: AtomicI32 = AtomicI32::new(0);

extern "C" fn forward_signal(sig: libc::c_int) {
    // Async-signal-safe: atomic load + `kill`.
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid > 0 {
        unsafe {
            libc::kill(pid, sig);
        }
    }
}

fn install_signal_forwarder(pid: i32) -> Result<()> {
    CHILD_PID.store(pid, Ordering::SeqCst);
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        let prev = unsafe { libc::signal(sig, forward_signal as *const () as libc::sighandler_t) };
        if prev == libc::SIG_ERR {
            anyhow::bail!("installing handler for signal {sig} failed");
        }
    }
    Ok(())
}

fn clear_signal_forwarder() {
    CHILD_PID.store(0, Ordering::SeqCst);
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe {
            libc::signal(sig, libc::SIG_DFL);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_at(t: u64, utime: u64, stime: u64) -> (u64, u64, RawSample) {
        (
            t,
            0,
            RawSample {
                utime_ticks: utime,
                stime_ticks: stime,
                rss_bytes: 1,
                vsz_bytes: 1,
                threads: 1,
                open_fds: 1,
                io_read_bytes: 0,
                io_write_bytes: 0,
            },
        )
    }

    #[test]
    fn compute_samples_first_sample_has_zero_cpu() {
        let raw = vec![raw_at(0, 0, 0)];
        let s = compute_samples(&raw);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].cpu_user_pct, 0.0);
        assert_eq!(s[0].cpu_sys_pct, 0.0);
    }

    #[test]
    fn compute_samples_pegging_one_core_is_about_100_pct() {
        // 1 second elapsed, exactly CLK_TCK user ticks burned → 100% user CPU.
        let ticks_per_sec = clock_ticks_per_second() as u64;
        let raw = vec![raw_at(0, 0, 0), raw_at(1000, ticks_per_sec, 0)];
        let s = compute_samples(&raw);
        assert!(
            (s[1].cpu_user_pct - 100.0).abs() < 0.5,
            "expected ~100%, got {}",
            s[1].cpu_user_pct
        );
        assert!(s[1].cpu_sys_pct.abs() < 0.5);
    }

    #[test]
    fn compute_samples_handles_two_cores_pegged() {
        let ticks_per_sec = clock_ticks_per_second() as u64;
        // 500ms wall, 1 second of user ticks burned → 200% user CPU.
        let raw = vec![raw_at(0, 0, 0), raw_at(500, ticks_per_sec, 0)];
        let s = compute_samples(&raw);
        assert!(
            (s[1].cpu_user_pct - 200.0).abs() < 1.0,
            "expected ~200%, got {}",
            s[1].cpu_user_pct
        );
    }

    #[test]
    fn exit_status_zero_returns_zero() {
        assert_eq!(exit_status_to_u8(Some(0), None), 0);
    }

    #[test]
    fn exit_status_nonzero_truncates_to_u8() {
        assert_eq!(exit_status_to_u8(Some(42), None), 42);
        assert_eq!(exit_status_to_u8(Some(258), None), 2);
    }

    #[test]
    fn exit_status_signal_uses_128_plus_signum() {
        assert_eq!(exit_status_to_u8(None, Some(2)), 130); // SIGINT
        assert_eq!(exit_status_to_u8(None, Some(15)), 143); // SIGTERM
    }

    #[test]
    fn env_fingerprint_is_64_hex_chars() {
        let fp = env_fingerprint();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn resolve_output_path_uses_explicit_value() {
        let p = PathBuf::from("/tmp/foo.json");
        let r = resolve_output_path(Some(&p), &chrono::Utc::now()).unwrap();
        assert_eq!(r, p);
    }

    #[test]
    fn resolve_output_path_falls_back_to_rprof_dir() {
        let t = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();
        let r = resolve_output_path(None, &t).unwrap();
        assert!(r.starts_with(".rprof"));
        assert!(r.extension().and_then(|s| s.to_str()) == Some("json"));
    }

    #[test]
    fn format_bytes_picks_appropriate_unit() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MiB");
    }
}
