/* rprof viewer — turns inlined report JSON into interactive uPlot charts. */
(() => {
  "use strict";

  const data = JSON.parse(document.getElementById("rprof-data").textContent);
  /** @type {{label: string, report: object}[]} */
  const runs = data.runs;

  const PALETTE = [
    "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728",
    "#9467bd", "#8c564b", "#e377c2", "#7f7f7f"
  ];

  // ---------- helpers ----------
  const escapeHtml = (s) =>
    String(s).replace(/[&<>"]/g, (c) => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;"
    }[c]));

  const fmtBytes = (b) => {
    if (b == null || !isFinite(b)) return "—";
    const a = Math.abs(b);
    if (a >= 1024 ** 3) return (b / 1024 ** 3).toFixed(2) + " GiB";
    if (a >= 1024 ** 2) return (b / 1024 ** 2).toFixed(1) + " MiB";
    if (a >= 1024) return (b / 1024).toFixed(1) + " KiB";
    return Math.round(b) + " B";
  };

  const fmtMs = (ms) => {
    if (ms == null) return "—";
    if (ms >= 60000) return (ms / 60000).toFixed(2) + " min";
    if (ms >= 1000) return (ms / 1000).toFixed(2) + " s";
    return ms + " ms";
  };

  const fmtPct = (p) => (p == null ? "—" : p.toFixed(1) + " %");
  const fmtNum = (n) => (n == null ? "—" : Number(n).toLocaleString());
  const fmtBytesPerSec = (b) => (b == null ? "—" : fmtBytes(b) + "/s");
  const fmtSeconds = (s) => (s == null ? "—" : s.toFixed(2) + " s");

  // ---------- summary table ----------
  const sumTbl = document.getElementById("summary");
  const header =
    "<tr>" +
    ["Run", "Command", "Wall", "Peak RSS", "User CPU", "System CPU", "Exit"]
      .map((h) => `<th>${h}</th>`)
      .join("") +
    "</tr>";
  const body = runs
    .map((r, i) => {
      const color = PALETTE[i % PALETTE.length];
      const exit =
        r.report.run.signal != null
          ? `signal ${r.report.run.signal}`
          : r.report.run.exit_code != null
          ? String(r.report.run.exit_code)
          : "—";
      return (
        "<tr>" +
        `<td><span class="swatch" style="background:${color}"></span>${escapeHtml(r.label)}</td>` +
        `<td><code>${escapeHtml(r.report.run.command.join(" "))}</code></td>` +
        `<td>${fmtMs(r.report.run.wall_duration_ms)}</td>` +
        `<td>${fmtBytes(r.report.summary.peak_rss_bytes)}</td>` +
        `<td>${fmtMs(r.report.summary.user_cpu_ms)}</td>` +
        `<td>${fmtMs(r.report.summary.system_cpu_ms)}</td>` +
        `<td>${escapeHtml(exit)}</td>` +
        "</tr>"
      );
    })
    .join("");
  sumTbl.innerHTML = header + body;

  // Pre-compute IO rates (derivative of cumulative bytes).
  runs.forEach((r) => {
    const s = r.report.samples;
    for (let i = 0; i < s.length; i++) {
      if (i === 0) {
        s[i]._io_read_rate = 0;
        s[i]._io_write_rate = 0;
      } else {
        const dt = (s[i].t_ms - s[i - 1].t_ms) / 1000;
        s[i]._io_read_rate =
          dt > 0
            ? Math.max(0, (s[i].io_read_bytes - s[i - 1].io_read_bytes) / dt)
            : 0;
        s[i]._io_write_rate =
          dt > 0
            ? Math.max(0, (s[i].io_write_bytes - s[i - 1].io_write_bytes) / dt)
            : 0;
      }
    }
  });

  // Build unified-X data: union of all t_ms across runs (in seconds).
  function buildData(seriesDefs) {
    const xs = Array.from(
      new Set(
        runs.flatMap((r) => r.report.samples.map((s) => s.t_ms))
      )
    ).sort((a, b) => a - b);
    const idx = new Map();
    xs.forEach((x, i) => idx.set(x, i));
    const xData = xs.map((t) => t / 1000);
    const ys = seriesDefs.map((d) => {
      const arr = new Array(xs.length).fill(null);
      d.run.report.samples.forEach((s) => {
        arr[idx.get(s.t_ms)] = d.fn(s);
      });
      return arr;
    });
    return [xData, ...ys];
  }

  const SYNC_KEY = "rprof";

  function plot(id, title, height, fmt, seriesDefs) {
    const el = document.getElementById(id);
    if (!el || seriesDefs.length === 0) return;
    const arr = buildData(seriesDefs);
    const opts = {
      title,
      width: el.clientWidth || 900,
      height,
      cursor: {
        focus: { prox: 8 },
        sync: { key: SYNC_KEY },
      },
      legend: { live: true },
      scales: { x: { time: false } },
      axes: [
        { label: "time (s)", values: (u, splits) => splits.map(fmtSeconds) },
        { values: (u, splits) => splits.map(fmt) },
      ],
      series: [
        { value: (u, v) => fmtSeconds(v) },
        ...seriesDefs.map((d) => ({
          label: d.label,
          stroke: d.stroke,
          dash: d.dash,
          width: 1.5,
          value: (u, v) => (v == null ? "—" : fmt(v)),
        })),
      ],
    };
    new uPlot(opts, arr, el);
  }

  // CPU: user (solid) + sys (dashed) per run.
  plot(
    "chart-cpu",
    "CPU % (user solid, system dashed)",
    260,
    fmtPct,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} user`,
        stroke: PALETTE[i % PALETTE.length],
        fn: (s) => s.cpu_user_pct,
      },
      {
        run: r,
        label: `${r.label} sys`,
        stroke: PALETTE[i % PALETTE.length],
        dash: [6, 4],
        fn: (s) => s.cpu_sys_pct,
      },
    ])
  );

  // Memory: RSS (solid) overlaid with VSZ (dashed) per run.
  plot(
    "chart-mem",
    "Memory (RSS solid, VSZ dashed)",
    260,
    fmtBytes,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} RSS`,
        stroke: PALETTE[i % PALETTE.length],
        fn: (s) => s.rss_bytes,
      },
      {
        run: r,
        label: `${r.label} VSZ`,
        stroke: PALETTE[i % PALETTE.length],
        dash: [6, 4],
        fn: (s) => s.vsz_bytes,
      },
    ])
  );

  plot(
    "chart-threads",
    "Threads",
    160,
    fmtNum,
    runs.map((r, i) => ({
      run: r,
      label: r.label,
      stroke: PALETTE[i % PALETTE.length],
      fn: (s) => s.threads,
    }))
  );

  plot(
    "chart-fds",
    "Open file descriptors",
    160,
    fmtNum,
    runs.map((r, i) => ({
      run: r,
      label: r.label,
      stroke: PALETTE[i % PALETTE.length],
      fn: (s) => s.open_fds,
    }))
  );

  plot(
    "chart-io",
    "IO rate (read solid, write dashed)",
    220,
    fmtBytesPerSec,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} read`,
        stroke: PALETTE[i % PALETTE.length],
        fn: (s) => s._io_read_rate,
      },
      {
        run: r,
        label: `${r.label} write`,
        stroke: PALETTE[i % PALETTE.length],
        dash: [6, 4],
        fn: (s) => s._io_write_rate,
      },
    ])
  );
})();
