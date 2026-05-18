// SPDX-License-Identifier: MIT

//! JSON Lines on-disk schema (revision 1) for rprof reports.
//!
//! A report is a UTF-8 text file containing one JSON object per line. The
//! first line is a `header` carrying the schema revision plus run/host
//! metadata; zero or more `sample` lines follow, in ascending `t_ms`
//! order; an optional final `footer` line carries values only known after
//! the child exits.
//!
//! See `requirements/schema-v1.md` for the authoritative specification.
//! The canonical example in that file is embedded below as a string
//! constant and parsed by `canonical_example_parses` so the documented
//! shape cannot silently drift from the writer.

use serde::{Deserialize, Serialize};

/// The on-disk schema revision. A reader that does not recognise the
/// header's `schema` value refuses the file. Adding optional fields is
/// forward-compatible and does **not** bump this number.
pub const SCHEMA_VERSION: u32 = 1;

/// File extension for on-disk reports. The format is JSON Lines.
pub const REPORT_EXTENSION: &str = "jsonl";

/// One record line in a report. Serialised with a `"type"` discriminant
/// (`"header"`, `"sample"`, or `"footer"`); unknown discriminants on read
/// produce a deserialisation error which the line-by-line loader treats
/// as "skip" (forward compatibility with future record kinds).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Record {
    Header(Header),
    Sample(Sample),
    Footer(Footer),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Header {
    pub schema: u32,
    pub tool: Tool,
    pub run: Run,
    pub host: Host,
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
    /// RFC 3339 / ISO 8601 with millisecond precision, in UTC
    /// (e.g. `2026-05-14T10:30:00.000Z`).
    pub start_time: String,
    /// Sampling backend identifier (e.g. `proc` for Linux /proc polling).
    pub backend: String,
    /// Requested poll interval in milliseconds.
    pub sample_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Host {
    pub hostname: String,
    pub kernel: String,
    pub cpu_count: u32,
    pub total_memory_bytes: u64,
    /// Platform `sysconf(_SC_CLK_TCK)`. Readers use this to convert
    /// per-sample `utime_ticks` / `stime_ticks` to seconds. Carried once
    /// on the header rather than on every sample.
    pub clock_ticks_per_sec: u64,
}

/// One sample record. CPU is recorded as **cumulative ticks**; rates and
/// percentages are the reader's job. Every field is self-contained — no
/// sample's content depends on a previous sample — so a truncated file
/// remains parseable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub t_ms: u64,
    pub wall_ms: u64,
    pub utime_ticks: u64,
    pub stime_ticks: u64,
    pub rss_bytes: u64,
    pub vsz_bytes: u64,
    pub threads: u32,
    pub open_fds: u32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Footer {
    pub wall_duration_ms: u64,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub user_cpu_ms: u64,
    pub system_cpu_ms: u64,
}

/// Canonical v1 report example, also reproduced verbatim in
/// `requirements/schema-v1.md`. Embedded here so the example in the
/// requirement is kept honest by a unit test (`canonical_example_parses`)
/// that parses it.
#[cfg(test)]
const CANONICAL_EXAMPLE_JSONL: &str = "\
{\"type\":\"header\",\"schema\":1,\"tool\":{\"name\":\"rprof\",\"version\":\"0.1.0\"},\"run\":{\"command\":[\"sleep\",\"0.1\"],\"cwd\":\"/tmp\",\"start_time\":\"2026-05-14T10:30:00.000Z\",\"backend\":\"proc\",\"sample_interval_ms\":100},\"host\":{\"hostname\":\"h\",\"kernel\":\"Linux 6.8.0\",\"cpu_count\":4,\"total_memory_bytes\":17179869184,\"clock_ticks_per_sec\":100}}
{\"type\":\"sample\",\"t_ms\":0,\"wall_ms\":1700000000000,\"utime_ticks\":0,\"stime_ticks\":0,\"rss_bytes\":1048576,\"vsz_bytes\":2097152,\"threads\":1,\"open_fds\":4,\"io_read_bytes\":0,\"io_write_bytes\":0}
{\"type\":\"footer\",\"wall_duration_ms\":100,\"exit_code\":0,\"signal\":null,\"user_cpu_ms\":12,\"system_cpu_ms\":3}
";

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> Header {
        Header {
            schema: SCHEMA_VERSION,
            tool: Tool::current(),
            run: Run {
                command: vec!["sleep".into(), "0.1".into()],
                cwd: "/tmp".into(),
                start_time: "2026-05-14T10:30:00.000Z".into(),
                backend: "proc".into(),
                sample_interval_ms: 100,
            },
            host: Host {
                hostname: "h".into(),
                kernel: "Linux 6.8.0".into(),
                cpu_count: 4,
                total_memory_bytes: 17_179_869_184,
                clock_ticks_per_sec: 100,
            },
        }
    }

    fn sample_sample() -> Sample {
        Sample {
            t_ms: 0,
            wall_ms: 1_700_000_000_000,
            utime_ticks: 0,
            stime_ticks: 0,
            rss_bytes: 1_048_576,
            vsz_bytes: 2_097_152,
            threads: 1,
            open_fds: 4,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }

    fn sample_footer() -> Footer {
        Footer {
            wall_duration_ms: 100,
            exit_code: Some(0),
            signal: None,
            user_cpu_ms: 12,
            system_cpu_ms: 3,
        }
    }

    // Requirements: schema-v1
    #[test]
    fn header_sample_footer_roundtrip_line_by_line() {
        let recs = [
            ("header", Record::Header(sample_header())),
            ("sample", Record::Sample(sample_sample())),
            ("footer", Record::Footer(sample_footer())),
        ];
        for (name, rec) in recs {
            let json = serde_json::to_string(&rec).unwrap();
            assert!(
                !json.contains('\n'),
                "{name} serialisation must be single-line, got: {json}"
            );
            assert!(
                json.contains(&format!("\"type\":\"{name}\"")),
                "{name} must include type tag: {json}"
            );
            let back: Record =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name} must reparse: {e}"));
            assert_eq!(back, rec, "{name} round-trip changed value");
        }
    }

    // Requirements: schema-v1
    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    // Requirements: schema-v1
    #[test]
    fn canonical_example_parses() {
        // The same JSONL document appears in requirements/schema-v1.md. If
        // this test fails after editing the requirement, update one or the
        // other so they stay in sync.
        let lines: Vec<&str> = CANONICAL_EXAMPLE_JSONL
            .lines()
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 3, "header + sample + footer");

        match serde_json::from_str::<Record>(lines[0]).expect("header parses") {
            Record::Header(h) => {
                assert_eq!(h.schema, SCHEMA_VERSION);
                assert_eq!(h.tool.name, "rprof");
                assert_eq!(h.run.command, vec!["sleep", "0.1"]);
                assert_eq!(h.run.backend, "proc");
                assert_eq!(h.host.cpu_count, 4);
                assert_eq!(h.host.clock_ticks_per_sec, 100);
            }
            other => panic!("expected header, got {other:?}"),
        }
        match serde_json::from_str::<Record>(lines[1]).expect("sample parses") {
            Record::Sample(s) => {
                assert_eq!(s.t_ms, 0);
                assert_eq!(s.utime_ticks, 0);
                assert_eq!(s.rss_bytes, 1_048_576);
            }
            other => panic!("expected sample, got {other:?}"),
        }
        match serde_json::from_str::<Record>(lines[2]).expect("footer parses") {
            Record::Footer(f) => {
                assert_eq!(f.wall_duration_ms, 100);
                assert_eq!(f.exit_code, Some(0));
                assert_eq!(f.signal, None);
                assert_eq!(f.user_cpu_ms, 12);
                assert_eq!(f.system_cpu_ms, 3);
            }
            other => panic!("expected footer, got {other:?}"),
        }
    }

    // Requirements: schema-v1
    #[test]
    fn additive_fields_tolerated_on_read() {
        // An extra field on the run object must not break the parser.
        let line = r#"{"type":"header","schema":1,"tool":{"name":"rprof","version":"0.1.0"},"run":{"command":["echo"],"cwd":"/tmp","start_time":"2026-05-14T10:30:00Z","backend":"proc","sample_interval_ms":100,"future_field":"ignored"},"host":{"hostname":"h","kernel":"Linux","cpu_count":1,"total_memory_bytes":0,"clock_ticks_per_sec":100}}"#;
        let r: Result<Record, _> = serde_json::from_str(line);
        assert!(r.is_ok(), "extra fields should be tolerated: {r:?}");
    }

    // Requirements: schema-v1
    #[test]
    fn unknown_record_type_is_skipped_by_reader() {
        // Lines whose `type` is unknown must not deserialise as a Record.
        // The line-by-line loader treats this as a skip, not as an error.
        let line = r#"{"type":"chart","x":1}"#;
        let r: Result<Record, _> = serde_json::from_str(line);
        assert!(r.is_err(), "unknown record kind should not deserialise");
    }
}
