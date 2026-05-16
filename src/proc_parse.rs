// SPDX-License-Identifier: MIT

//! Parsers for `/proc/<pid>/{stat,statm,status,io}` and helpers that read
//! file descriptors from `/proc/<pid>/fd`.
//!
//! All parsers operate on string slices (or byte slices for `stat`) so they
//! can be exercised with fixture data in unit tests.

use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Subset of `/proc/<pid>/stat` we care about. The file is space-separated
/// but field 2 (`comm`) is wrapped in parentheses and may contain spaces or
/// closing parens, so we slice out the final `)` and parse the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcStat {
    /// Field 14 — user time in clock ticks.
    pub utime_ticks: u64,
    /// Field 15 — system time in clock ticks.
    pub stime_ticks: u64,
    /// Field 20 — number of threads.
    pub num_threads: u32,
    /// Field 23 — virtual memory size in bytes.
    pub vsize_bytes: u64,
    /// Field 24 — resident set size in pages. Multiply by page size.
    pub rss_pages: u64,
}

impl ProcStat {
    /// Parse the contents of `/proc/<pid>/stat`.
    pub fn parse(contents: &str) -> Result<Self> {
        // Split off the comm field. `man 5 proc` recommends finding the last `)`.
        let close = contents
            .rfind(')')
            .ok_or_else(|| anyhow!("missing closing `)` in /proc/.../stat"))?;
        let rest = &contents[close + 1..];
        // Fields after comm are space-separated. Field 3 in the spec (`state`)
        // is now at index 0 of `rest_fields`.
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // We need up to spec-field 24, which is index 21 here.
        if fields.len() < 22 {
            return Err(anyhow!(
                "expected at least 22 fields after comm, got {}",
                fields.len()
            ));
        }
        let utime_ticks: u64 = fields[11].parse().context("parse utime")?;
        let stime_ticks: u64 = fields[12].parse().context("parse stime")?;
        let num_threads: u32 = fields[17].parse().context("parse num_threads")?;
        let vsize_bytes: u64 = fields[20].parse().context("parse vsize")?;
        let rss_pages: u64 = fields[21].parse().context("parse rss")?;
        Ok(Self {
            utime_ticks,
            stime_ticks,
            num_threads,
            vsize_bytes,
            rss_pages,
        })
    }
}

/// Bytes read/written, parsed from `/proc/<pid>/io`. The file is missing for
/// processes the caller cannot ptrace, in which case the parser returns
/// zeros — io accounting is best-effort.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProcIo {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

impl ProcIo {
    pub fn parse(contents: &str) -> Result<Self> {
        let mut read_bytes = 0u64;
        let mut write_bytes = 0u64;
        for line in contents.lines() {
            if let Some(v) = line.strip_prefix("read_bytes:") {
                read_bytes = v
                    .trim()
                    .parse()
                    .with_context(|| format!("parse read_bytes from `{line}`"))?;
            } else if let Some(v) = line.strip_prefix("write_bytes:") {
                write_bytes = v
                    .trim()
                    .parse()
                    .with_context(|| format!("parse write_bytes from `{line}`"))?;
            }
        }
        Ok(Self {
            read_bytes,
            write_bytes,
        })
    }
}

/// Count entries in `/proc/<pid>/fd`. Returns 0 if the directory is gone.
pub fn count_fds(fd_dir: &Path) -> Result<u32> {
    let read = match std::fs::read_dir(fd_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e).context(format!("read_dir {}", fd_dir.display())),
    };
    let mut n = 0u32;
    for entry in read {
        match entry {
            Ok(_) => n += 1,
            // Entries can disappear between read_dir and stat — that's fine.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("iterate /proc/<pid>/fd"),
        }
    }
    Ok(n)
}

/// Children PIDs from `/proc/<pid>/task/*/children`. Each task directory has a
/// `children` file with a space-separated list. We don't enumerate task IDs
/// up front because new tasks can appear at any time.
pub fn read_children(proc_pid_dir: &Path) -> Result<Vec<u32>> {
    let task_dir = proc_pid_dir.join("task");
    let read = match std::fs::read_dir(&task_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context(format!("read_dir {}", task_dir.display())),
    };
    let mut out = Vec::new();
    for entry in read {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let children_file = entry.path().join("children");
        let contents = match std::fs::read_to_string(&children_file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for tok in contents.split_whitespace() {
            if let Ok(pid) = tok.parse::<u32>() {
                out.push(pid);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Realistic stat line. `bash` PID 1234, several fields. Position-counted
    /// so the indices in [`ProcStat::parse`] are exercised. utime=400,
    /// stime=50, threads=3, vsize=12345678, rss=1024.
    const STAT_FIXTURE: &str = "1234 (bash) S 1000 1234 1234 0 -1 4194304 100 200 0 0 \
        400 50 0 0 20 0 3 0 1000 12345678 1024 \
        18446744073709551615 1 1 0 0 0 0 0 0 65536 1 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0";

    #[test]
    fn parse_proc_stat_extracts_cpu_threads_memory() {
        let s = ProcStat::parse(STAT_FIXTURE).unwrap();
        assert_eq!(s.utime_ticks, 400);
        assert_eq!(s.stime_ticks, 50);
        assert_eq!(s.num_threads, 3);
        assert_eq!(s.vsize_bytes, 12_345_678);
        assert_eq!(s.rss_pages, 1024);
    }

    #[test]
    fn parse_proc_stat_handles_comm_with_spaces() {
        // comm field contains a space and a `)`. Splitter must find the LAST `)`.
        let s = "9999 (weird ) name) R 1 1 1 0 -1 0 0 0 0 0 \
            7 8 0 0 20 0 1 0 1000 4096 2 \
            0 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0";
        let parsed = ProcStat::parse(s).unwrap();
        assert_eq!(parsed.utime_ticks, 7);
        assert_eq!(parsed.stime_ticks, 8);
        assert_eq!(parsed.num_threads, 1);
        assert_eq!(parsed.vsize_bytes, 4096);
        assert_eq!(parsed.rss_pages, 2);
    }

    #[test]
    fn parse_proc_stat_rejects_truncated() {
        assert!(ProcStat::parse("1234 (sh) S 1 2 3").is_err());
    }

    #[test]
    fn parse_proc_io_reads_read_and_write_bytes() {
        let txt = "\
rchar: 100
wchar: 200
syscr: 10
syscw: 20
read_bytes: 4096
write_bytes: 8192
cancelled_write_bytes: 0
";
        let io = ProcIo::parse(txt).unwrap();
        assert_eq!(io.read_bytes, 4096);
        assert_eq!(io.write_bytes, 8192);
    }

    #[test]
    fn parse_proc_io_defaults_to_zero_when_fields_absent() {
        let io = ProcIo::parse("rchar: 0\nwchar: 0\n").unwrap();
        assert_eq!(io, ProcIo::default());
    }

    #[test]
    fn count_fds_counts_directory_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let fd = tmp.path().join("fd");
        std::fs::create_dir(&fd).unwrap();
        std::fs::write(fd.join("0"), "").unwrap();
        std::fs::write(fd.join("1"), "").unwrap();
        std::fs::write(fd.join("2"), "").unwrap();
        assert_eq!(count_fds(&fd).unwrap(), 3);
    }

    #[test]
    fn count_fds_returns_zero_for_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert_eq!(count_fds(&missing).unwrap(), 0);
    }

    #[test]
    fn read_children_aggregates_across_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let task = tmp.path().join("task");
        std::fs::create_dir(&task).unwrap();
        let t1 = task.join("100");
        std::fs::create_dir(&t1).unwrap();
        std::fs::write(t1.join("children"), "200 201 ").unwrap();
        let t2 = task.join("101");
        std::fs::create_dir(&t2).unwrap();
        std::fs::write(t2.join("children"), "300\n").unwrap();

        let mut got = read_children(tmp.path()).unwrap();
        got.sort();
        assert_eq!(got, vec![200, 201, 300]);
    }

    #[test]
    fn read_children_returns_empty_when_task_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let got = read_children(tmp.path()).unwrap();
        assert!(got.is_empty());
    }
}
