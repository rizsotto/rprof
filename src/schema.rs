// SPDX-License-Identifier: MIT

//! JSON schema for rprof reports.
//!
//! The schema is per-sample (one object per timestamp) — this is more verbose
//! than a columnar layout but keeps memory usage low during capture and is
//! trivially streamable.
//!
//! `schema_version` is an integer; viewers reject unknown majors. Additive
//! fields do not bump the version, so unknown fields are tolerated on read.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub schema_version: u32,
    pub tool: Tool,
    pub run: Run,
    pub host: Host,
    pub summary: Summary,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    pub version: String,
}

impl Tool {
    pub fn current() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub command: Vec<String>,
    pub cwd: String,
    /// SHA-256 hex digest of the sorted KEY=VALUE environment list. We
    /// deliberately do not store the full environment.
    pub env_fingerprint: String,
    /// Wall-clock start time, ISO 8601 / RFC 3339.
    pub start_time: String,
    pub wall_duration_ms: u64,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub backend: String,
    pub sample_interval_ms: u64,
    /// Whether process-tree aggregation was on. Direct child only by default.
    pub include_children: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Host {
    pub hostname: String,
    pub kernel: String,
    pub cpu_count: u32,
    pub total_memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub peak_rss_bytes: u64,
    pub user_cpu_ms: u64,
    pub system_cpu_ms: u64,
    pub sample_count: u64,
}

/// One sample point.
///
/// `t_ms` is a monotonic offset from the start of the run.
/// `wall_ms` is the wall-clock millisecond offset since the unix epoch — kept
/// so two runs can be aligned on absolute time when diffing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub t_ms: u64,
    pub wall_ms: u64,
    pub cpu_user_pct: f64,
    pub cpu_sys_pct: f64,
    pub rss_bytes: u64,
    pub vsz_bytes: u64,
    pub threads: u32,
    pub open_fds: u32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrips_through_json() {
        let r = Report {
            schema_version: SCHEMA_VERSION,
            tool: Tool::current(),
            run: Run {
                command: vec!["echo".into(), "hello".into()],
                cwd: "/tmp".into(),
                env_fingerprint: "0".repeat(64),
                start_time: "2026-05-14T10:30:00Z".into(),
                wall_duration_ms: 1000,
                exit_code: Some(0),
                signal: None,
                backend: "proc".into(),
                sample_interval_ms: 100,
                include_children: false,
            },
            host: Host {
                hostname: "h".into(),
                kernel: "Linux 6.8.0".into(),
                cpu_count: 4,
                total_memory_bytes: 1 << 32,
            },
            summary: Summary {
                peak_rss_bytes: 1 << 20,
                user_cpu_ms: 12,
                system_cpu_ms: 3,
                sample_count: 1,
            },
            samples: vec![Sample {
                t_ms: 0,
                wall_ms: 1_700_000_000_000,
                cpu_user_pct: 1.0,
                cpu_sys_pct: 0.5,
                rss_bytes: 1024,
                vsz_bytes: 2048,
                threads: 1,
                open_fds: 4,
                io_read_bytes: 0,
                io_write_bytes: 0,
            }],
        };

        let json = serde_json::to_string(&r).unwrap();
        let back: Report = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn additive_fields_tolerated_on_read() {
        // A report with an extra field at the run level should still parse.
        let json = r#"{
            "schema_version": 1,
            "tool": {"name": "rprof", "version": "0.1.0"},
            "run": {
                "command": ["echo"],
                "cwd": "/tmp",
                "env_fingerprint": "00",
                "start_time": "2026-05-14T10:30:00Z",
                "wall_duration_ms": 0,
                "exit_code": 0,
                "signal": null,
                "backend": "proc",
                "sample_interval_ms": 100,
                "include_children": false,
                "future_field": "ignored"
            },
            "host": {"hostname": "h", "kernel": "Linux", "cpu_count": 1, "total_memory_bytes": 0},
            "summary": {"peak_rss_bytes": 0, "user_cpu_ms": 0, "system_cpu_ms": 0, "sample_count": 0},
            "samples": []
        }"#;
        let r: Result<Report, _> = serde_json::from_str(json);
        assert!(r.is_ok(), "extra fields should be tolerated: {r:?}");
    }
}
