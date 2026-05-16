/* rprof viewer — turns inlined report JSON into interactive uPlot charts. */
(() => {
  "use strict";

  const data = JSON.parse(document.getElementById("rprof-data").textContent);
  /** @type {{label: string, report: object}[]} */
  const runs = data.runs;

  // Paired colors (Tableau 20): [dark, light] per run. Dark is the primary
  // metric (RSS, user CPU, read), light is the secondary on the same chart
  // (VSZ, system CPU, write). Same hue per run keeps a run's series
  // visually grouped across charts.
  const PALETTE = [
    ["#0d4f8b", "#1f77b4"], // blue
    ["#b3590a", "#ff7f0e"], // orange
    ["#1f7020", "#2ca02c"], // green
    ["#971b1c", "#d62728"], // red
    ["#684685", "#9467bd"], // purple
    ["#5d3a31", "#8c564b"], // brown
    ["#a14787", "#e377c2"], // pink
    ["#525252", "#7f7f7f"]  // gray
  ];
  const colorFor = (i, variant) => PALETTE[i % PALETTE.length][variant];

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
      const color = colorFor(i, 0);
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
    // Render our own title above the plot rather than using uPlot's so we
    // can tighten its spacing to the plot (and away from the next chart).
    const titleEl = document.createElement("div");
    titleEl.className = "chart-title";
    const titleText = document.createElement("span");
    titleText.textContent = title;
    const toggleBtn = document.createElement("button");
    toggleBtn.type = "button";
    toggleBtn.className = "chart-toggle";
    toggleBtn.textContent = "−";
    toggleBtn.setAttribute("aria-label", "Hide chart");
    toggleBtn.addEventListener("click", () => {
      const collapsed = el.classList.toggle("collapsed");
      toggleBtn.textContent = collapsed ? "+" : "−";
      toggleBtn.setAttribute("aria-label", collapsed ? "Show chart" : "Hide chart");
    });
    titleEl.appendChild(titleText);
    titleEl.appendChild(toggleBtn);
    el.appendChild(titleEl);
    // The chart container has horizontal padding for the surrounding border;
    // subtract it so the plot canvas fits inside the padded area.
    const cs = getComputedStyle(el);
    const padX =
      (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0);
    const arr = buildData(seriesDefs);
    const opts = {
      width: (el.clientWidth || 900) - padX,
      height,
      cursor: {
        focus: { prox: 8 },
        sync: { key: SYNC_KEY },
        // Runs sample at independent timestamps, so at most x positions only
        // one run has a real value and the others are null. Snap each series
        // to its own nearest non-null sample so the legend shows every run
        // simultaneously instead of "value / —".
        dataIdx: (u, seriesIdx, hoveredIdx) => {
          if (seriesIdx === 0) return hoveredIdx;
          const ys = u.data[seriesIdx];
          if (ys[hoveredIdx] != null) return hoveredIdx;
          let left = hoveredIdx - 1;
          let right = hoveredIdx + 1;
          while (left >= 0 || right < ys.length) {
            if (left >= 0 && ys[left] != null) {
              if (right < ys.length && ys[right] != null) {
                return hoveredIdx - left <= right - hoveredIdx ? left : right;
              }
              return left;
            }
            if (right < ys.length && ys[right] != null) return right;
            left--;
            right++;
          }
          return hoveredIdx;
        },
      },
      legend: { live: true },
      scales: { x: { time: false } },
      axes: [
        { label: "time (s)", values: (u, splits) => splits.map(fmtSeconds) },
        { values: (u, splits) => splits.map(fmt) },
      ],
      series: [
        { label: "time", value: (u, v) => fmtSeconds(v) },
        ...seriesDefs.map((d) => ({
          label: d.label,
          stroke: d.stroke,
          width: 1.5,
          // Each run only has data at its own sample timestamps; nulls at the
          // other run's timestamps would otherwise break the line into dots.
          spanGaps: true,
          value: (u, v) => (v == null ? "—" : fmt(v)),
        })),
      ],
    };
    new uPlot(opts, arr, el);
  }

  const CHART_HEIGHT = 300;

  // CPU: user (dark) + sys (light) per run.
  plot(
    "chart-cpu",
    "CPU %",
    CHART_HEIGHT,
    fmtPct,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} user`,
        stroke: colorFor(i, 0),
        fn: (s) => s.cpu_user_pct,
      },
      {
        run: r,
        label: `${r.label} sys`,
        stroke: colorFor(i, 1),
        fn: (s) => s.cpu_sys_pct,
      },
    ])
  );

  // Memory: RSS (dark) + VSZ (light) per run.
  plot(
    "chart-mem",
    "Memory",
    CHART_HEIGHT,
    fmtBytes,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} RSS`,
        stroke: colorFor(i, 0),
        fn: (s) => s.rss_bytes,
      },
      {
        run: r,
        label: `${r.label} VSZ`,
        stroke: colorFor(i, 1),
        fn: (s) => s.vsz_bytes,
      },
    ])
  );

  plot(
    "chart-threads",
    "Threads",
    CHART_HEIGHT,
    fmtNum,
    runs.map((r, i) => ({
      run: r,
      label: r.label,
      stroke: colorFor(i, 0),
      fn: (s) => s.threads,
    }))
  );

  plot(
    "chart-fds",
    "Open file descriptors",
    CHART_HEIGHT,
    fmtNum,
    runs.map((r, i) => ({
      run: r,
      label: r.label,
      stroke: colorFor(i, 0),
      fn: (s) => s.open_fds,
    }))
  );

  // IO: read (dark) + write (light) per run.
  plot(
    "chart-io",
    "IO rate",
    CHART_HEIGHT,
    fmtBytesPerSec,
    runs.flatMap((r, i) => [
      {
        run: r,
        label: `${r.label} read`,
        stroke: colorFor(i, 0),
        fn: (s) => s._io_read_rate,
      },
      {
        run: r,
        label: `${r.label} write`,
        stroke: colorFor(i, 1),
        fn: (s) => s._io_write_rate,
      },
    ])
  );
})();
