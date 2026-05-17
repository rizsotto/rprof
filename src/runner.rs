// SPDX-License-Identifier: MIT

//! `rprof run` implementation.
//!
//! Spawns the user's command with inherited stdio, polls `/proc/<pid>` on a
//! background thread at the configured interval, forwards SIGINT/SIGTERM to
//! the child, and **streams** the report to disk as JSONL records: a
//! `header` immediately after spawn, then one `sample` per tick, then a
//! `footer` once the child has exited and been reaped.
//!
//! The streaming model means a SIGKILL'd `rprof` (or a host power loss)
//! leaves a partial-but-usable file rather than nothing. See
//! `requirements/capture-streaming-write.md` and
//! `requirements/schema-v1.md` for the contract.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use sha2::Digest;

use crate::cli::RunArgs;
use crate::schema::{
    Footer, Header, Host, Record, Run, Sample, Tool, REPORT_EXTENSION, SCHEMA_VERSION,
};

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
        run_impl(args)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        unreachable!()
    }
}

fn run_impl(args: RunArgs) -> Result<u8> {
    use crate::sampler::{ProcSampler, Sampler};

    let (program, child_args) = args
        .command
        .split_first()
        .expect("non-empty command checked above");

    let timing = RunTiming::now();
    let output_path = resolve_output_path(args.output.as_ref(), &timing.start_wall)?;
    let mut writer = ReportWriter::create(&output_path)?;

    let mut child = Command::new(program)
        .args(child_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn {program:?}"))?;
    let child_pid = child.id();

    install_signal_forwarder(child_pid as i32)?;

    // Header is emitted before the first sample (the streaming-write
    // contract). A reader that opens the file at this instant sees a
    // header and zero samples — that is a valid partial report.
    writer.record_header(build_header(&args, &timing))?;

    let (footer_tx, footer_rx) = channel::<Footer>();
    let interval = args.interval;

    // The sampler thread owns the writer for the entire run. The main
    // thread sends the assembled `Footer` down `footer_tx` once
    // `child.wait()` returns; the sampler appends it and exits. Keeping
    // the writer on a single thread removes the ferry-back across the
    // join boundary that the previous shape needed just to write the
    // last record.
    let sampler_handle: thread::JoinHandle<Result<()>> = thread::spawn(move || {
        let mut sampler: Box<dyn Sampler> = Box::new(ProcSampler::new(child_pid));
        loop {
            let t_ms = timing.start_instant.elapsed().as_millis() as u64;
            let wall_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            match sampler.sample()? {
                Some(raw) => writer.record_sample(build_sample(t_ms, wall_ms, &raw))?,
                None => {
                    // Child vanished before main observed `wait()` returning.
                    // Block on the channel so the footer the main thread is
                    // about to assemble still reaches the file.
                    if let Ok(footer) = footer_rx.recv() {
                        writer.record_footer(footer)?;
                    }
                    return Ok(());
                }
            }
            match footer_rx.recv_timeout(interval) {
                Ok(footer) => {
                    writer.record_footer(footer)?;
                    return Ok(());
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    });

    let status = child.wait().context("waiting for child to exit")?;
    clear_signal_forwarder();

    let footer = build_footer(&status, &timing);
    // If `send` fails the sampler already exited with an error; `join`
    // below surfaces it.
    let _ = footer_tx.send(footer.clone());

    sampler_handle
        .join()
        .map_err(|_| anyhow::anyhow!("sampler thread panicked"))??;

    eprintln!(
        "rprof: wrote {} (wall {}ms, cpu_user {}ms, cpu_sys {}ms, exit {})",
        output_path.display(),
        footer.wall_duration_ms,
        footer.user_cpu_ms,
        footer.system_cpu_ms,
        match (footer.exit_code, footer.signal) {
            (Some(c), _) => format!("{c}"),
            (None, Some(s)) => format!("signal {s}"),
            _ => "unknown".to_string(),
        },
    );

    Ok(exit_status_to_u8(footer.exit_code, footer.signal))
}

/// The two clocks the runner keeps in sync: the wall clock (used for
/// the header's `start_time` and the output-file name) and the
/// monotonic clock (used for per-sample `t_ms` and the footer's
/// `wall_duration_ms`). One value travels through the run instead of
/// loose locals on each thread.
#[derive(Clone, Copy)]
struct RunTiming {
    start_wall: chrono::DateTime<chrono::Utc>,
    start_instant: Instant,
}

impl RunTiming {
    fn now() -> Self {
        Self {
            start_wall: chrono::Utc::now(),
            start_instant: Instant::now(),
        }
    }

    fn start_time_rfc3339(&self) -> String {
        self.start_wall
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    fn elapsed_ms(&self) -> u64 {
        self.start_instant.elapsed().as_millis() as u64
    }
}

fn build_header(args: &RunArgs, timing: &RunTiming) -> Header {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    Header {
        schema: SCHEMA_VERSION,
        tool: Tool::current(),
        run: Run {
            command: args.command.clone(),
            cwd,
            env_fingerprint: env_fingerprint(),
            start_time: timing.start_time_rfc3339(),
            backend: "proc".to_string(),
            sample_interval_ms: args.interval.as_millis() as u64,
        },
        host: host_metadata(clock_ticks_per_second_u64()),
    }
}

fn build_footer(status: &ExitStatus, timing: &RunTiming) -> Footer {
    let (user_cpu_ms, system_cpu_ms) = read_rusage_children();
    let (exit_code, signal) = decode_status(status);
    Footer {
        wall_duration_ms: timing.elapsed_ms(),
        exit_code,
        signal,
        user_cpu_ms,
        system_cpu_ms,
    }
}

/// Streaming JSONL writer for an on-disk report. Owns the buffered
/// file handle and flushes after every record, so the file on disk
/// always trails the most recent sample by at most the time it takes
/// to serialise one line — see `requirements/capture-streaming-write.md`.
struct ReportWriter {
    inner: BufWriter<File>,
}

impl ReportWriter {
    /// Create or truncate the report at `path`, creating the parent
    /// directory if necessary. The internal buffer is intentionally
    /// small: the per-record flush surrenders only the current line on
    /// SIGKILL, never an accumulated backlog.
    fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating output directory {}", parent.display()))?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("opening report {}", path.display()))?;
        Ok(Self {
            inner: BufWriter::with_capacity(8 * 1024, file),
        })
    }

    fn record_header(&mut self, header: Header) -> Result<()> {
        self.write_record(&Record::Header(header))
    }

    fn record_sample(&mut self, sample: Sample) -> Result<()> {
        self.write_record(&Record::Sample(sample))
    }

    fn record_footer(&mut self, footer: Footer) -> Result<()> {
        self.write_record(&Record::Footer(footer))
    }

    fn write_record(&mut self, rec: &Record) -> Result<()> {
        let mut buf = serde_json::to_vec(rec).context("serialising record")?;
        buf.push(b'\n');
        self.inner.write_all(&buf).context("writing record")?;
        self.inner.flush().context("flushing record")?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn build_sample(t_ms: u64, wall_ms: u64, raw: &crate::sampler::RawSample) -> Sample {
    Sample {
        t_ms,
        wall_ms,
        utime_ticks: raw.utime_ticks,
        stime_ticks: raw.stime_ticks,
        rss_bytes: raw.rss_bytes,
        vsz_bytes: raw.vsz_bytes,
        threads: raw.threads,
        open_fds: raw.open_fds,
        io_read_bytes: raw.io_read_bytes,
        io_write_bytes: raw.io_write_bytes,
    }
}

fn clock_ticks_per_second_u64() -> u64 {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: `sysconf` is thread-safe.
        let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if v > 0 {
            v as u64
        } else {
            100
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        100
    }
}

fn env_fingerprint() -> String {
    let mut entries: Vec<String> = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    entries.sort();
    let joined = entries.join("\n");
    let digest = sha2::Sha256::digest(joined.as_bytes());
    hex::encode(digest)
}

#[cfg(target_os = "linux")]
fn host_metadata(clock_ticks_per_sec: u64) -> Host {
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
        clock_ticks_per_sec,
    }
}

#[cfg(not(target_os = "linux"))]
fn host_metadata(clock_ticks_per_sec: u64) -> Host {
    Host {
        hostname: String::new(),
        kernel: String::new(),
        cpu_count: 0,
        total_memory_bytes: 0,
        clock_ticks_per_sec,
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
            Ok(dir.join(format!("{ts}.{REPORT_EXTENSION}")))
        }
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
        let p = PathBuf::from("/tmp/foo.jsonl");
        let r = resolve_output_path(Some(&p), &chrono::Utc::now()).unwrap();
        assert_eq!(r, p);
    }

    // Requirements: capture-output-path
    #[test]
    fn resolve_output_path_falls_back_to_rprof_dir_with_jsonl_extension() {
        let t = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();
        let r = resolve_output_path(None, &t).unwrap();
        assert!(r.starts_with(".rprof"));
        assert_eq!(r.extension().and_then(|s| s.to_str()), Some("jsonl"));
    }
}
