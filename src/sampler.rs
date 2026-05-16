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
    use std::collections::HashSet;
    use std::path::PathBuf;

    use anyhow::{Context, Result};

    use super::{RawSample, Sampler};
    use crate::proc_parse::{count_fds, read_children, ProcIo, ProcStat};

    /// Polls `/proc/<pid>` for one process and, optionally, the full process
    /// tree rooted at it.
    pub struct ProcSampler {
        root_pid: u32,
        include_children: bool,
        page_size: u64,
    }

    impl ProcSampler {
        pub fn new(pid: u32, include_children: bool) -> Self {
            // SAFETY: `sysconf` is a thread-safe libc call.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            let page_size = if page_size > 0 {
                page_size as u64
            } else {
                4096
            };
            Self {
                root_pid: pid,
                include_children,
                page_size,
            }
        }

        fn sample_pid(&self, pid: u32) -> Result<Option<RawSample>> {
            let dir = PathBuf::from(format!("/proc/{pid}"));
            let stat_contents = match std::fs::read_to_string(dir.join("stat")) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => {
                    return Err(e).context(format!("read /proc/{pid}/stat"));
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

    impl Sampler for ProcSampler {
        fn name(&self) -> &'static str {
            "proc"
        }

        fn sample(&mut self) -> Result<Option<RawSample>> {
            let Some(root) = self.sample_pid(self.root_pid)? else {
                return Ok(None);
            };
            if !self.include_children {
                return Ok(Some(root));
            }
            let mut totals = root;
            let mut seen: HashSet<u32> = HashSet::new();
            seen.insert(self.root_pid);
            let mut stack = vec![self.root_pid];
            while let Some(pid) = stack.pop() {
                let children =
                    read_children(&PathBuf::from(format!("/proc/{pid}"))).unwrap_or_default();
                for c in children {
                    if !seen.insert(c) {
                        continue;
                    }
                    stack.push(c);
                    if let Some(s) = self.sample_pid(c)? {
                        totals.utime_ticks = totals.utime_ticks.saturating_add(s.utime_ticks);
                        totals.stime_ticks = totals.stime_ticks.saturating_add(s.stime_ticks);
                        totals.rss_bytes = totals.rss_bytes.saturating_add(s.rss_bytes);
                        totals.vsz_bytes = totals.vsz_bytes.saturating_add(s.vsz_bytes);
                        totals.threads = totals.threads.saturating_add(s.threads);
                        totals.open_fds = totals.open_fds.saturating_add(s.open_fds);
                        totals.io_read_bytes = totals.io_read_bytes.saturating_add(s.io_read_bytes);
                        totals.io_write_bytes =
                            totals.io_write_bytes.saturating_add(s.io_write_bytes);
                    }
                }
            }
            Ok(Some(totals))
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn proc_sampler_returns_self_metrics() {
        let mut s = ProcSampler::new(std::process::id(), false);
        let raw = s.sample().unwrap().expect("self pid must be sampleable");
        assert!(raw.vsz_bytes > 0, "vsz should be non-zero");
        assert!(raw.rss_bytes > 0, "rss should be non-zero");
        assert!(raw.threads >= 1);
        assert_eq!(s.name(), "proc");
    }

    #[test]
    fn proc_sampler_returns_none_for_missing_pid() {
        // PID_MAX on Linux is 4194304; this won't exist.
        let mut s = ProcSampler::new(u32::MAX - 1, false);
        let raw = s.sample().unwrap();
        assert!(raw.is_none(), "missing pid should be None");
    }

    #[test]
    fn proc_sampler_include_children_does_not_fail_for_leaf_process() {
        // The test process has no children of its own most of the time; the
        // tree walk must still succeed and produce a non-zero sample.
        let pid = std::process::id();
        let mut s = ProcSampler::new(pid, true);
        let raw = s.sample().unwrap().expect("self pid must be sampleable");
        assert!(raw.vsz_bytes > 0);
        assert!(raw.rss_bytes > 0);
        assert!(raw.threads >= 1);
    }
}
