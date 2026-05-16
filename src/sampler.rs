// SPDX-License-Identifier: MIT

//! Backend-agnostic sampler interface.
//!
//! A [`Sampler`] produces a [`RawSample`] on each tick. Computing CPU
//! percentages and IO rates requires the previous sample, so that is done at
//! the runner level — the sampler itself returns cumulative counters.

use anyhow::Result;

/// One raw sample read from a backend. CPU times are in clock ticks
/// (`clock_t`), memory is in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RawSample {
    pub utime_ticks: u64,
    pub stime_ticks: u64,
    pub rss_bytes: u64,
    pub vsz_bytes: u64,
    pub threads: u32,
    pub open_fds: u32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// Per-tick sampler. `sample` returns `Ok(None)` when the target has exited
/// — the polling loop uses that as a graceful stop signal.
pub trait Sampler: Send {
    fn sample(&mut self) -> Result<Option<RawSample>>;
    fn name(&self) -> &'static str;
}

#[cfg(target_os = "linux")]
pub use proc_backend::ProcSampler;

#[cfg(target_os = "linux")]
mod proc_backend {
    use std::path::PathBuf;

    use anyhow::{Context, Result};

    use super::{RawSample, Sampler};
    use crate::proc_parse::{count_fds, ProcIo, ProcStat};

    /// Polls `/proc/<pid>` for one process.
    pub struct ProcSampler {
        pid: u32,
        page_size: u64,
    }

    impl ProcSampler {
        pub fn new(pid: u32) -> Self {
            // SAFETY: `sysconf` is a thread-safe libc call.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            let page_size = if page_size > 0 {
                page_size as u64
            } else {
                4096
            };
            Self { pid, page_size }
        }
    }

    impl Sampler for ProcSampler {
        fn name(&self) -> &'static str {
            "proc"
        }

        fn sample(&mut self) -> Result<Option<RawSample>> {
            let dir = PathBuf::from(format!("/proc/{}", self.pid));
            let stat_contents = match std::fs::read_to_string(dir.join("stat")) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => {
                    return Err(e).context(format!("read /proc/{}/stat", self.pid));
                }
            };
            let stat = ProcStat::parse(&stat_contents)?;
            let io = match std::fs::read_to_string(dir.join("io")) {
                Ok(s) => ProcIo::parse(&s).unwrap_or_default(),
                Err(_) => ProcIo::default(),
            };
            let fds = count_fds(&dir.join("fd")).unwrap_or(0);
            Ok(Some(RawSample {
                utime_ticks: stat.utime_ticks,
                stime_ticks: stat.stime_ticks,
                rss_bytes: stat.rss_pages.saturating_mul(self.page_size),
                vsz_bytes: stat.vsize_bytes,
                threads: stat.num_threads,
                open_fds: fds,
                io_read_bytes: io.read_bytes,
                io_write_bytes: io.write_bytes,
            }))
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    // Requirements: capture-proc-backend
    #[test]
    fn proc_sampler_returns_self_metrics() {
        let mut s = ProcSampler::new(std::process::id());
        let raw = s.sample().unwrap().expect("self pid must be sampleable");
        assert!(raw.vsz_bytes > 0, "vsz should be non-zero");
        assert!(raw.rss_bytes > 0, "rss should be non-zero");
        assert!(raw.threads >= 1);
        assert_eq!(s.name(), "proc");
    }

    // Requirements: capture-proc-backend
    #[test]
    fn proc_sampler_returns_none_for_missing_pid() {
        // PID_MAX on Linux is 4194304; this won't exist.
        let mut s = ProcSampler::new(u32::MAX - 1);
        let raw = s.sample().unwrap();
        assert!(raw.is_none(), "missing pid should be None");
    }
}
