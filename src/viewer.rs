// SPDX-License-Identifier: MIT

//! `rprof view` implementation. Renders one or more JSONL reports as a
//! self-contained HTML file with inlined uPlot charts.
//!
//! The on-disk format is JSON Lines (one record per line). The viewer
//! reads each line in order, tolerates unknown record types and missing
//! or truncated final lines, and refuses files whose `header.schema`
//! does not match `SCHEMA_VERSION`.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::ViewArgs;
use crate::schema::{Footer, Header, Record, Sample};

/// uPlot 1.6.31 — MIT licensed. Vendored under `assets/` and embedded at build
/// time so the viewer needs no network access and produces a single-file HTML.
const UPLOT_JS: &str = include_str!("../assets/uPlot.iife.min.js");
const UPLOT_CSS: &str = include_str!("../assets/uPlot.min.css");
const VIEWER_JS: &str = include_str!("../assets/viewer.js");
const VIEWER_CSS: &str = include_str!("../assets/viewer.css");

pub fn run(args: ViewArgs) -> Result<u8> {
    let entries = collect_inputs(args.reports, args.labels)?;
    if entries.is_empty() {
        anyhow::bail!(
            "no reports provided. Pass one or more report paths, or use --label LABEL:PATH."
        );
    }
    let loaded = load_reports(&entries)?;
    let html = render_html(&loaded)?;

    if args.no_open {
        match args.output.as_deref() {
            Some(p) => write_file(p, &html)?,
            None => std::io::stdout()
                .write_all(html.as_bytes())
                .context("writing HTML to stdout")?,
        }
        return Ok(0);
    }

    let path = match args.output.as_deref() {
        Some(p) => {
            write_file(p, &html)?;
            p.to_path_buf()
        }
        None => write_temp_html(&html)?,
    };
    open_in_browser(&path);
    Ok(0)
}

/// One loaded report: the header plus every parsed sample plus the footer
/// (if the file ended cleanly with one). A partial report — common after a
/// SIGKILL — has `footer = None`; the viewer renders the samples that did
/// land on disk.
#[derive(Debug)]
pub struct LoadedReport {
    pub header: Header,
    pub samples: Vec<Sample>,
    pub footer: Option<Footer>,
}

/// Pairing of a friendly label and the report it came from.
pub struct Loaded {
    pub label: String,
    pub report: LoadedReport,
}

/// Merge positional and `--label` arguments into a single ordered list. Labels
/// keyed by a path that also appears positionally override the auto-generated
/// filename label; labels for paths not listed positionally are appended.
fn collect_inputs(
    positional: Vec<PathBuf>,
    labels: Vec<(String, PathBuf)>,
) -> Result<Vec<(String, PathBuf)>> {
    use std::collections::HashMap;
    let mut label_map: HashMap<PathBuf, String> =
        labels.iter().cloned().map(|(l, p)| (p, l)).collect();
    let mut out: Vec<(String, PathBuf)> = Vec::with_capacity(positional.len() + labels.len());
    for p in &positional {
        let label = label_map.remove(p).unwrap_or_else(|| filename_label(p));
        out.push((label, p.clone()));
    }
    for (lbl, path) in labels {
        if !out.iter().any(|(_, p)| p == &path) {
            out.push((lbl, path));
        }
    }
    Ok(out)
}

fn filename_label(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_else(|| p.display().to_string())
}

fn load_reports(entries: &[(String, PathBuf)]) -> Result<Vec<Loaded>> {
    let mut out = Vec::with_capacity(entries.len());
    for (label, path) in entries {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let report = parse_jsonl(&text, path)?;
        out.push(Loaded {
            label: label.clone(),
            report,
        });
    }
    Ok(out)
}

/// Parse a JSONL report. The contract:
///
/// - First well-formed record must be a `header` whose `schema` matches
///   [`crate::schema::SCHEMA_VERSION`]. If not, the call fails with an
///   error mentioning the file path and `schema`.
/// - Subsequent `sample` records are collected in encounter order.
/// - At most one `footer` is recognised; once seen, further records are
///   not consulted.
/// - Lines whose `type` is unknown are silently skipped.
/// - A truncated or otherwise unparseable trailing line is tolerated and
///   simply means the run was interrupted before the line was finished.
///   Earlier malformed lines also turn into a skip rather than an error —
///   the streaming-write contract is "best effort to disk", so any
///   half-line on the way is treated as a survivor of a kill.
pub fn parse_jsonl(text: &str, path: &Path) -> Result<LoadedReport> {
    let mut header: Option<Header> = None;
    let mut samples: Vec<Sample> = Vec::new();
    let mut footer: Option<Footer> = None;

    for line in text.split_inclusive('\n') {
        // A trailing partial line (no terminating \n) is the streaming
        // contract's truncation case — skip without erroring.
        let complete = line.ends_with('\n');
        let trimmed = line.trim_matches(|c: char| c == '\n' || c == '\r');
        if trimmed.is_empty() {
            continue;
        }
        let rec = match serde_json::from_str::<Record>(trimmed) {
            Ok(r) => r,
            Err(_) => {
                // Unknown record types and partial / corrupt lines are
                // both invisible to the reader. We do not distinguish:
                // the schema mandates tolerance in both directions.
                if !complete {
                    break;
                }
                continue;
            }
        };
        match rec {
            Record::Header(h) => {
                if header.is_some() {
                    // A second header would mean a malformed file; the
                    // schema only allows one. Skip rather than fail to
                    // honour the "tolerate partial files" contract.
                    continue;
                }
                if h.schema != crate::schema::SCHEMA_VERSION {
                    anyhow::bail!(
                        "report {} uses schema {} but this rprof only understands {}",
                        path.display(),
                        h.schema,
                        crate::schema::SCHEMA_VERSION
                    );
                }
                header = Some(h);
            }
            Record::Sample(s) => samples.push(s),
            Record::Footer(f) => {
                if footer.is_none() {
                    footer = Some(f);
                }
            }
        }
    }

    let header = header.ok_or_else(|| {
        anyhow::anyhow!(
            "report {} is missing a header record (schema v{} expected)",
            path.display(),
            crate::schema::SCHEMA_VERSION
        )
    })?;
    Ok(LoadedReport {
        header,
        samples,
        footer,
    })
}

/// Build the self-contained HTML page.
pub fn render_html(loaded: &[Loaded]) -> Result<String> {
    let payload = build_payload(loaded)?;
    let title = page_title(loaded);
    let subtitle = page_subtitle(loaded);
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{title}</title><style>{uplot_css}\n{viewer_css}</style></head><body>\
<h1>{title}</h1><p class=\"subtitle\">{subtitle}</p>\
<table id=\"summary\"></table>\
<div id=\"chart-cpu\" class=\"chart\"></div>\
<div id=\"chart-mem\" class=\"chart\"></div>\
<div id=\"chart-threads\" class=\"chart\"></div>\
<div id=\"chart-fds\" class=\"chart\"></div>\
<div id=\"chart-io\" class=\"chart\"></div>\
<script id=\"rprof-data\" type=\"application/json\">{payload}</script>\
<script>{uplot_js}</script>\
<script>{viewer_js}</script>\
</body></html>",
        title = html_escape(&title),
        subtitle = html_escape(&subtitle),
        uplot_css = UPLOT_CSS,
        viewer_css = VIEWER_CSS,
        payload = payload,
        uplot_js = UPLOT_JS,
        viewer_js = VIEWER_JS,
    ))
}

fn page_title(loaded: &[Loaded]) -> String {
    if loaded.len() == 1 {
        format!("rprof — {}", loaded[0].label)
    } else {
        format!("rprof — {} runs", loaded.len())
    }
}

fn page_subtitle(loaded: &[Loaded]) -> String {
    if loaded.is_empty() {
        return String::new();
    }
    let first = &loaded[0].report.header;
    format!(
        "schema v{} · backend {} · interval {} ms",
        first.schema, first.run.backend, first.run.sample_interval_ms
    )
}

/// Reader-side per-sample view: cumulative ticks resolved into the CPU%
/// numbers the legend renders. This is the cross-sample work the schema
/// requires the reader (not the writer) to do.
#[derive(serde::Serialize)]
struct ViewSample {
    t_ms: u64,
    wall_ms: u64,
    cpu_user_pct: f64,
    cpu_sys_pct: f64,
    rss_bytes: u64,
    vsz_bytes: u64,
    threads: u32,
    open_fds: u32,
    io_read_bytes: u64,
    io_write_bytes: u64,
}

#[derive(serde::Serialize)]
struct ViewSummary {
    peak_rss_bytes: u64,
    user_cpu_ms: u64,
    system_cpu_ms: u64,
    sample_count: u64,
}

#[derive(serde::Serialize)]
struct ViewRun<'a> {
    command: &'a [String],
    cwd: &'a str,
    env_fingerprint: &'a str,
    start_time: &'a str,
    wall_duration_ms: u64,
    exit_code: Option<i32>,
    signal: Option<i32>,
    backend: &'a str,
    sample_interval_ms: u64,
}

#[derive(serde::Serialize)]
struct ViewReport<'a> {
    schema_version: u32,
    tool: &'a crate::schema::Tool,
    run: ViewRun<'a>,
    host: ViewHost<'a>,
    summary: ViewSummary,
    samples: Vec<ViewSample>,
}

#[derive(serde::Serialize)]
struct ViewHost<'a> {
    hostname: &'a str,
    kernel: &'a str,
    cpu_count: u32,
    total_memory_bytes: u64,
}

#[derive(serde::Serialize)]
struct ViewEntry<'a> {
    label: &'a str,
    report: ViewReport<'a>,
}

#[derive(serde::Serialize)]
struct ViewPayload<'a> {
    runs: Vec<ViewEntry<'a>>,
}

/// Derive a viewer-shaped report from the on-disk records. CPU% per
/// sample is computed as `(delta_ticks / clock_ticks_per_sec) /
/// delta_seconds * 100`; the first sample reads 0 % (no previous sample
/// to delta against). `peak_rss_bytes` is the maximum across samples.
fn build_view_report<'a>(label: &'a str, report: &'a LoadedReport) -> ViewEntry<'a> {
    let clk_tck = if report.header.host.clock_ticks_per_sec > 0 {
        report.header.host.clock_ticks_per_sec as f64
    } else {
        100.0
    };
    let mut samples = Vec::with_capacity(report.samples.len());
    for (i, s) in report.samples.iter().enumerate() {
        let (cpu_user_pct, cpu_sys_pct) = if i == 0 {
            (0.0, 0.0)
        } else {
            let prev = &report.samples[i - 1];
            let dt_ms = s.t_ms.saturating_sub(prev.t_ms).max(1);
            let dt_s = dt_ms as f64 / 1000.0;
            let du = s.utime_ticks.saturating_sub(prev.utime_ticks) as f64 / clk_tck;
            let ds = s.stime_ticks.saturating_sub(prev.stime_ticks) as f64 / clk_tck;
            ((du / dt_s) * 100.0, (ds / dt_s) * 100.0)
        };
        samples.push(ViewSample {
            t_ms: s.t_ms,
            wall_ms: s.wall_ms,
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
    let peak_rss = samples.iter().map(|s| s.rss_bytes).max().unwrap_or(0);
    let sample_count = samples.len() as u64;
    let footer = report.footer.as_ref();
    ViewEntry {
        label,
        report: ViewReport {
            schema_version: crate::schema::SCHEMA_VERSION,
            tool: &report.header.tool,
            run: ViewRun {
                command: &report.header.run.command,
                cwd: &report.header.run.cwd,
                env_fingerprint: &report.header.run.env_fingerprint,
                start_time: &report.header.run.start_time,
                wall_duration_ms: footer.map(|f| f.wall_duration_ms).unwrap_or(0),
                exit_code: footer.and_then(|f| f.exit_code),
                signal: footer.and_then(|f| f.signal),
                backend: &report.header.run.backend,
                sample_interval_ms: report.header.run.sample_interval_ms,
            },
            host: ViewHost {
                hostname: &report.header.host.hostname,
                kernel: &report.header.host.kernel,
                cpu_count: report.header.host.cpu_count,
                total_memory_bytes: report.header.host.total_memory_bytes,
            },
            summary: ViewSummary {
                peak_rss_bytes: peak_rss,
                user_cpu_ms: footer.map(|f| f.user_cpu_ms).unwrap_or(0),
                system_cpu_ms: footer.map(|f| f.system_cpu_ms).unwrap_or(0),
                sample_count,
            },
            samples,
        },
    }
}

/// Serialise the loaded runs into the JSON blob the viewer JS consumes.
///
/// Escapes `</` to `<\/` so the payload can safely live inside a
/// `<script type="application/json">` element.
fn build_payload(loaded: &[Loaded]) -> Result<String> {
    let runs: Vec<ViewEntry> = loaded
        .iter()
        .map(|l| build_view_report(&l.label, &l.report))
        .collect();
    let p = ViewPayload { runs };
    let s = serde_json::to_string(&p).context("serializing viewer payload")?;
    Ok(s.replace("</", "<\\/"))
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn write_file(path: &Path, html: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
    }
    std::fs::write(path, html).with_context(|| format!("writing HTML to {}", path.display()))
}

fn write_temp_html(html: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir();
    let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f");
    let pid = std::process::id();
    let path = dir.join(format!("rprof-{stamp}-{pid}.html"));
    write_file(&path, html)?;
    Ok(path)
}

fn open_in_browser(path: &Path) {
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &["open"];
    #[cfg(target_os = "linux")]
    let candidates: &[&str] = &["xdg-open"];
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let candidates: &[&str] = &[];

    for opener in candidates {
        let res = std::process::Command::new(opener)
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if let Ok(status) = res {
            if status.success() {
                eprintln!("rprof: opened {} in browser", path.display());
                return;
            }
        }
    }
    eprintln!(
        "rprof: report written to {} (could not auto-open a browser)",
        path.display()
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Footer, Header, Host, Run, Sample, Tool, SCHEMA_VERSION};

    fn fake_header(cmd: &str) -> Header {
        Header {
            schema: SCHEMA_VERSION,
            tool: Tool::current(),
            run: Run {
                command: vec![cmd.into()],
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
        }
    }

    fn fake_sample(t_ms: u64) -> Sample {
        Sample {
            t_ms,
            wall_ms: 0,
            utime_ticks: 0,
            stime_ticks: 0,
            rss_bytes: 1024,
            vsz_bytes: 2048,
            threads: 1,
            open_fds: 3,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }

    fn fake_loaded(label: &str, cmd: &str) -> Loaded {
        Loaded {
            label: label.into(),
            report: LoadedReport {
                header: fake_header(cmd),
                samples: vec![fake_sample(0)],
                footer: Some(Footer {
                    wall_duration_ms: 500,
                    exit_code: Some(0),
                    signal: None,
                    user_cpu_ms: 0,
                    system_cpu_ms: 0,
                }),
            },
        }
    }

    // Requirements: viewer-chart-inventory
    #[test]
    fn render_html_emits_complete_document() {
        let html = render_html(&[fake_loaded("only", "echo")]).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("uPlot"), "uPlot bundle should be embedded");
        assert!(html.contains("id=\"rprof-data\""));
        assert!(html.contains("\"label\":\"only\""));
        let positions: Vec<_> = [
            "id=\"chart-cpu\"",
            "id=\"chart-mem\"",
            "id=\"chart-threads\"",
            "id=\"chart-fds\"",
            "id=\"chart-io\"",
        ]
        .iter()
        .map(|id| html.find(id).expect("chart id present"))
        .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "chart containers should appear in inventory order"
        );
    }

    // Requirements: viewer-chart-interaction
    #[test]
    fn render_html_carries_interaction_machinery() {
        let html = render_html(&[fake_loaded("only", "echo")]).unwrap();
        assert!(
            html.contains("sync"),
            "cursor sync configuration must be present"
        );
        assert!(
            html.contains("chart-toggle"),
            "collapse toggle class must be referenced"
        );
        assert!(
            html.contains("tabular-nums"),
            "legend tabular-numerals styling must be present"
        );
    }

    #[test]
    fn render_html_inlines_multiple_runs() {
        let html =
            render_html(&[fake_loaded("before", "old"), fake_loaded("after", "new")]).unwrap();
        assert!(html.contains("\"label\":\"before\""));
        assert!(html.contains("\"label\":\"after\""));
        assert!(html.contains("2 runs"));
    }

    #[test]
    fn payload_escapes_closing_script_tags() {
        let mut r = fake_loaded("x", "ok");
        r.report.header.run.command = vec!["bash".into(), "-c".into(), "echo </script>".into()];
        let html = render_html(&[r]).unwrap();
        assert!(!html.contains("</script>echo"));
        assert!(html.contains("<\\/script>"));
    }

    #[test]
    fn collect_inputs_uses_filename_when_no_label() {
        let got = collect_inputs(
            vec![
                PathBuf::from("/tmp/foo.jsonl"),
                PathBuf::from("/tmp/bar.jsonl"),
            ],
            vec![],
        )
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "foo");
        assert_eq!(got[1].0, "bar");
    }

    #[test]
    fn collect_inputs_label_overrides_positional() {
        let got = collect_inputs(
            vec![PathBuf::from("/tmp/foo.jsonl")],
            vec![("before".into(), PathBuf::from("/tmp/foo.jsonl"))],
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "before");
    }

    #[test]
    fn collect_inputs_label_only_is_accepted() {
        let got = collect_inputs(
            vec![],
            vec![
                ("a".into(), PathBuf::from("/tmp/a.jsonl")),
                ("b".into(), PathBuf::from("/tmp/b.jsonl")),
            ],
        )
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "a");
        assert_eq!(got[1].0, "b");
    }

    #[test]
    fn collect_inputs_preserves_positional_order() {
        let got = collect_inputs(
            vec![PathBuf::from("z.jsonl"), PathBuf::from("a.jsonl")],
            vec![],
        )
        .unwrap();
        assert_eq!(got[0].1, PathBuf::from("z.jsonl"));
        assert_eq!(got[1].1, PathBuf::from("a.jsonl"));
    }

    // Requirements: schema-v1
    #[test]
    fn parse_jsonl_accepts_header_only_partial_file() {
        // A run killed before any sample was taken still has a header.
        let header = serde_json::to_string(&Record::Header(fake_header("echo"))).unwrap();
        let text = format!("{header}\n");
        let r = parse_jsonl(&text, Path::new("/tmp/x.jsonl")).unwrap();
        assert!(r.samples.is_empty());
        assert!(r.footer.is_none());
    }

    // Requirements: schema-v1
    #[test]
    fn parse_jsonl_tolerates_unknown_record_types() {
        let header = serde_json::to_string(&Record::Header(fake_header("echo"))).unwrap();
        let sample = serde_json::to_string(&Record::Sample(fake_sample(0))).unwrap();
        // Insert an unknown record type between header and sample.
        let text = format!("{header}\n{{\"type\":\"future\",\"x\":1}}\n{sample}\n");
        let r = parse_jsonl(&text, Path::new("/tmp/x.jsonl")).unwrap();
        assert_eq!(r.samples.len(), 1);
    }

    // Requirements: schema-v1
    #[test]
    fn parse_jsonl_tolerates_truncated_final_line() {
        let header = serde_json::to_string(&Record::Header(fake_header("echo"))).unwrap();
        let sample = serde_json::to_string(&Record::Sample(fake_sample(0))).unwrap();
        // Final line is missing the closing brace and the trailing newline.
        let mut partial = sample.clone();
        partial.truncate(partial.len() - 5);
        let text = format!("{header}\n{sample}\n{partial}");
        let r = parse_jsonl(&text, Path::new("/tmp/x.jsonl")).unwrap();
        // Only the well-formed sample is kept; the truncated tail is ignored.
        assert_eq!(r.samples.len(), 1);
        assert!(r.footer.is_none());
    }

    // Requirements: schema-v1
    #[test]
    fn parse_jsonl_rejects_unknown_schema_with_path_and_field() {
        let bad = r#"{"type":"header","schema":999,"tool":{"name":"rprof","version":"x"},"run":{"command":["x"],"cwd":"/","env_fingerprint":"00","start_time":"2026-01-01T00:00:00Z","backend":"proc","sample_interval_ms":100},"host":{"hostname":"h","kernel":"x","cpu_count":1,"total_memory_bytes":0,"clock_ticks_per_sec":100}}"#;
        let text = format!("{bad}\n");
        let err = parse_jsonl(&text, Path::new("/tmp/bad.jsonl")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("/tmp/bad.jsonl"), "error mentions path: {msg}");
        assert!(msg.contains("schema"), "error mentions schema: {msg}");
    }

    // Requirements: schema-v1, capture-streaming-write
    #[test]
    fn parse_jsonl_recovers_footer_when_present() {
        let header = serde_json::to_string(&Record::Header(fake_header("echo"))).unwrap();
        let sample = serde_json::to_string(&Record::Sample(fake_sample(0))).unwrap();
        let footer = serde_json::to_string(&Record::Footer(Footer {
            wall_duration_ms: 250,
            exit_code: Some(0),
            signal: None,
            user_cpu_ms: 5,
            system_cpu_ms: 1,
        }))
        .unwrap();
        let text = format!("{header}\n{sample}\n{footer}\n");
        let r = parse_jsonl(&text, Path::new("/tmp/x.jsonl")).unwrap();
        assert_eq!(r.samples.len(), 1);
        let f = r.footer.expect("footer present");
        assert_eq!(f.wall_duration_ms, 250);
        assert_eq!(f.exit_code, Some(0));
    }

    // Requirements: capture-cpu-pct
    #[test]
    fn build_view_report_zeroes_first_sample_cpu_and_computes_subsequent() {
        let mut header = fake_header("echo");
        header.host.clock_ticks_per_sec = 100;
        let s0 = Sample {
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
        };
        let s1 = Sample {
            t_ms: 1000,
            // 100 user ticks in 1s at 100 Hz → 100% user CPU.
            utime_ticks: 100,
            ..s0.clone()
        };
        let report = LoadedReport {
            header,
            samples: vec![s0, s1],
            footer: None,
        };
        let entry = build_view_report("only", &report);
        assert_eq!(entry.report.samples[0].cpu_user_pct, 0.0);
        assert_eq!(entry.report.samples[0].cpu_sys_pct, 0.0);
        assert!(
            (entry.report.samples[1].cpu_user_pct - 100.0).abs() < 0.5,
            "expected ~100%, got {}",
            entry.report.samples[1].cpu_user_pct
        );
    }

    // Requirements: capture-peak-rss-accuracy
    #[test]
    fn build_view_report_takes_max_rss_across_samples() {
        let header = fake_header("echo");
        let mut s0 = fake_sample(0);
        s0.rss_bytes = 1024;
        let mut s1 = fake_sample(100);
        s1.rss_bytes = 4096;
        let mut s2 = fake_sample(200);
        s2.rss_bytes = 2048;
        let report = LoadedReport {
            header,
            samples: vec![s0, s1, s2],
            footer: None,
        };
        let entry = build_view_report("only", &report);
        assert_eq!(entry.report.summary.peak_rss_bytes, 4096);
        assert_eq!(entry.report.summary.sample_count, 3);
    }
}
