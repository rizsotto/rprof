//! `rprof view` implementation. Renders one or more JSON reports as a
//! self-contained HTML file with inlined uPlot charts.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::ViewArgs;
use crate::schema::Report;

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
            "no reports provided. Pass one or more JSON paths, or use --label LABEL:PATH."
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

/// Pairing of a friendly label and the report it came from.
pub struct Loaded {
    pub label: String,
    pub report: Report,
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
        let bytes =
            std::fs::read(path).with_context(|| format!("reading report {}", path.display()))?;
        let report: Report = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing report {}", path.display()))?;
        if report.schema_version != crate::schema::SCHEMA_VERSION {
            anyhow::bail!(
                "report {} uses schema_version {} but this rprof only understands {}",
                path.display(),
                report.schema_version,
                crate::schema::SCHEMA_VERSION
            );
        }
        out.push(Loaded {
            label: label.clone(),
            report,
        });
    }
    Ok(out)
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
    let first = &loaded[0].report;
    format!(
        "schema v{} · backend {} · interval {} ms",
        first.schema_version, first.run.backend, first.run.sample_interval_ms
    )
}

/// Serialize the loaded runs into the JSON blob the viewer JS consumes.
///
/// Escapes `</` to `<\/` so the payload can safely live inside a
/// `<script type="application/json">` element.
fn build_payload(loaded: &[Loaded]) -> Result<String> {
    #[derive(serde::Serialize)]
    struct RunEntry<'a> {
        label: &'a str,
        report: &'a Report,
    }
    #[derive(serde::Serialize)]
    struct Payload<'a> {
        runs: Vec<RunEntry<'a>>,
    }
    let p = Payload {
        runs: loaded
            .iter()
            .map(|l| RunEntry {
                label: &l.label,
                report: &l.report,
            })
            .collect(),
    };
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
    use crate::schema::{Host, Run, Sample, Summary, Tool, SCHEMA_VERSION};

    fn fake_report(cmd: &str) -> Report {
        Report {
            schema_version: SCHEMA_VERSION,
            tool: Tool::current(),
            run: Run {
                command: vec![cmd.into()],
                cwd: "/tmp".into(),
                env_fingerprint: "0".repeat(64),
                start_time: "2026-05-14T10:30:00Z".into(),
                wall_duration_ms: 500,
                exit_code: Some(0),
                signal: None,
                backend: "proc".into(),
                sample_interval_ms: 100,
                include_children: false,
            },
            host: Host {
                hostname: "h".into(),
                kernel: "Linux".into(),
                cpu_count: 1,
                total_memory_bytes: 0,
            },
            summary: Summary {
                peak_rss_bytes: 1024,
                user_cpu_ms: 0,
                system_cpu_ms: 0,
                sample_count: 1,
            },
            samples: vec![Sample {
                t_ms: 0,
                wall_ms: 0,
                cpu_user_pct: 0.0,
                cpu_sys_pct: 0.0,
                rss_bytes: 1024,
                vsz_bytes: 2048,
                threads: 1,
                open_fds: 3,
                io_read_bytes: 0,
                io_write_bytes: 0,
            }],
        }
    }

    fn loaded(label: &str, cmd: &str) -> Loaded {
        Loaded {
            label: label.into(),
            report: fake_report(cmd),
        }
    }

    #[test]
    fn render_html_emits_complete_document() {
        let html = render_html(&[loaded("only", "echo")]).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("uPlot"), "uPlot bundle should be embedded");
        assert!(html.contains("id=\"rprof-data\""));
        assert!(html.contains("\"label\":\"only\""));
        assert!(html.contains("chart-cpu"));
        assert!(html.contains("chart-mem"));
        assert!(html.contains("chart-threads"));
        assert!(html.contains("chart-fds"));
        assert!(html.contains("chart-io"));
    }

    #[test]
    fn render_html_inlines_multiple_runs() {
        let html = render_html(&[loaded("before", "old"), loaded("after", "new")]).unwrap();
        assert!(html.contains("\"label\":\"before\""));
        assert!(html.contains("\"label\":\"after\""));
        assert!(html.contains("2 runs"));
    }

    #[test]
    fn payload_escapes_closing_script_tags() {
        // Build a report whose command line contains </script> — without
        // escaping, that would prematurely close the <script> element and
        // break the page.
        let mut r = loaded("x", "ok");
        r.report.run.command = vec!["bash".into(), "-c".into(), "echo </script>".into()];
        let html = render_html(&[r]).unwrap();
        assert!(!html.contains("</script>echo"));
        assert!(html.contains("<\\/script>"));
    }

    #[test]
    fn collect_inputs_uses_filename_when_no_label() {
        let got = collect_inputs(
            vec![
                PathBuf::from("/tmp/foo.json"),
                PathBuf::from("/tmp/bar.json"),
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
            vec![PathBuf::from("/tmp/foo.json")],
            vec![("before".into(), PathBuf::from("/tmp/foo.json"))],
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "before");
    }

    #[test]
    fn collect_inputs_label_only_is_accepted() {
        // No positional reports; the --label entry alone provides the path.
        let got = collect_inputs(
            vec![],
            vec![
                ("a".into(), PathBuf::from("/tmp/a.json")),
                ("b".into(), PathBuf::from("/tmp/b.json")),
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
            vec![PathBuf::from("z.json"), PathBuf::from("a.json")],
            vec![],
        )
        .unwrap();
        assert_eq!(got[0].1, PathBuf::from("z.json"));
        assert_eq!(got[1].1, PathBuf::from("a.json"));
    }
}
