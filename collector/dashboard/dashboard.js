/* A local, read-only view of aggregate rows returned by Wrangler. */
(() => {
  const data = window.FLUME_DASHBOARD_DATA;
  if (!data?.ranges) {
    document.querySelector(".main-content").textContent =
      "No dashboard snapshot was loaded. Run npm run dashboard from collector/.";
    return;
  }

  const number = new Intl.NumberFormat();
  const eventNames = {
    launched: "App launched",
    sessionEnded: "Session ended",
    libraryCount: "Library size reported",
    torrentPreviewed: "Torrent previewed",
    torrentAdded: "Torrent added",
    torrentCompleted: "Torrent completed",
    torrentRemoved: "Torrent removed",
    libraryImported: "Library imported",
    settingChanged: "Setting changed",
    operationFailed: "Operation failed",
  };
  const platformNames = { macos: "macOS", windows: "Windows", linux: "Linux" };
  const archNames = {
    aarch64: "ARM64",
    x86_64: "x86-64",
    arm: "ARM",
    x86: "x86",
  };
  const rangeNames = {
    7: "Last 7 days",
    30: "Last 30 days",
    90: "Last 90 days",
    all: "All recorded time",
  };
  const svgNS = "http://www.w3.org/2000/svg";
  let range = data.ranges["30"].summary.events > 0 ? "30" : "all";
  let metric = "events";

  const $ = (id) => document.getElementById(id);
  const fmt = (value) => number.format(value || 0);
  const utcDay = (seconds) =>
    new Date(seconds * 1000).toISOString().slice(0, 10);
  const addDays = (day, offset) => {
    const date = new Date(`${day}T00:00:00Z`);
    date.setUTCDate(date.getUTCDate() + offset);
    return date.toISOString().slice(0, 10);
  };
  const fmtDay = (day) =>
    new Date(`${day}T00:00:00Z`).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    });
  const fmtMonth = (month) =>
    new Date(`${month}-01T00:00:00Z`).toLocaleDateString(undefined, {
      month: "short",
      year: "numeric",
      timeZone: "UTC",
    });

  function setStaticMeta() {
    $("source-label").textContent =
      data.source === "local" ? "Local D1" : "Remote D1";
    $("generated-at").textContent = new Date(
      data.generatedAt * 1000,
    ).toLocaleString(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    });
    if (data.latestReceivedAt) {
      $("stat-last").textContent = new Date(
        data.latestReceivedAt * 1000,
      ).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "numeric",
        minute: "2-digit",
      });
      const ageDays = Math.floor(
        (data.generatedAt - data.latestReceivedAt) / 86400,
      );
      $("last-note").textContent =
        ageDays > 0
          ? `${ageDays} ${ageDays === 1 ? "day" : "days"} before snapshot · your timezone`
          : "Received within 24 hours · your timezone";
    } else {
      $("stat-last").textContent = "No reports";
      $("last-note").textContent = "Collector receipt time";
    }
  }

  function rowsForRange(report) {
    const byDay = new Map(report.daily.map((row) => [row.day, row]));
    let start;
    let end = utcDay(data.generatedAt);
    if (range === "all") {
      if (!report.summary.firstAt) return [];
      start = utcDay(report.summary.firstAt);
      if (report.summary.lastAt)
        end = [end, utcDay(report.summary.lastAt)].sort().at(-1);
    } else {
      start = addDays(end, 1 - Number(range));
    }
    const rows = [];
    for (let day = start; day <= end; day = addDays(day, 1)) {
      rows.push(byDay.get(day) ?? { day, events: 0, installs: 0 });
    }
    return rows;
  }

  function chartBuckets(rows) {
    if (rows.length <= 120) return { buckets: rows, monthly: false };
    const months = new Map();
    for (const row of rows) {
      const key = row.day.slice(0, 7);
      const bucket = months.get(key) ?? { day: key, events: 0, installs: 0 };
      bucket.events += row.events;
      // A monthly sum would count a reporting install more than once. Without
      // monthly distinct IDs in the report, show the peak observed day instead.
      bucket.installs = Math.max(bucket.installs, row.installs);
      months.set(key, bucket);
    }
    return { buckets: [...months.values()], monthly: true };
  }

  function svgElement(tag, attrs = {}) {
    const node = document.createElementNS(svgNS, tag);
    for (const [key, value] of Object.entries(attrs))
      node.setAttribute(key, String(value));
    return node;
  }

  function niceStep(max) {
    if (max <= 1) return 1;
    const raw = max / 4;
    const power = 10 ** Math.floor(Math.log10(raw));
    return (
      [1, 2, 5, 10].map((n) => n * power).find((n) => n >= raw) ?? power * 10
    );
  }

  function renderChart(rows) {
    const svg = $("trend-chart");
    svg.replaceChildren();
    const viewWidth = Math.max(
      240,
      Math.round(svg.getBoundingClientRect().width),
    );
    svg.setAttribute("viewBox", `0 0 ${viewWidth} 260`);
    const { buckets, monthly } = chartBuckets(rows);
    const values = buckets.map((row) => row[metric]);
    const max = Math.max(...values, 0);
    const step = niceStep(max);
    const ceiling = Math.max(step, Math.ceil(max / step) * step);
    const left = 42,
      right = 8,
      top = 15,
      bottom = 239;
    const width = viewWidth - left - right;
    const height = bottom - top;
    for (let tick = 0; tick <= ceiling; tick += step) {
      const y = bottom - (tick / ceiling) * height;
      svg.append(
        svgElement("line", {
          x1: left,
          x2: viewWidth - right,
          y1: y,
          y2: y,
          class: "grid-line",
        }),
      );
      const label = svgElement("text", {
        x: left - 9,
        y: y + 3.5,
        "text-anchor": "end",
        class: "axis-label",
      });
      label.textContent = fmt(tick);
      svg.append(label);
    }
    const slot = width / Math.max(buckets.length, 1);
    const barWidth = Math.min(18, Math.max(1, slot * 0.64));
    buckets.forEach((bucket, index) => {
      const value = bucket[metric];
      if (value === 0) return;
      const barHeight = Math.max(2, (value / ceiling) * height);
      const rect = svgElement("rect", {
        x: left + index * slot + (slot - barWidth) / 2,
        y: bottom - barHeight,
        width: barWidth,
        height: barHeight,
        rx: Math.min(2, barWidth / 3),
        class: `bar${metric === "installs" ? " installs" : ""}`,
      });
      const title = svgElement("title");
      title.textContent = `${monthly ? fmtMonth(bucket.day) : bucket.day}: ${fmt(value)} ${monthly && metric === "installs" ? "peak daily reporting installs" : metric}`;
      rect.append(title);
      svg.append(rect);
    });
    svg.setAttribute(
      "aria-label",
      `${monthly && metric === "installs" ? "Peak daily reporting installs" : metric === "events" ? "Events" : "Reporting installs"} by UTC ${monthly ? "month" : "day"}`,
    );
    $("chart-start").textContent = buckets.length
      ? monthly
        ? fmtMonth(buckets[0].day)
        : fmtDay(buckets[0].day)
      : "—";
    $("chart-end").textContent = buckets.length
      ? monthly
        ? fmtMonth(buckets.at(-1).day)
        : fmtDay(buckets.at(-1).day)
      : "—";
    $("chart-unit").textContent =
      monthly && metric === "installs"
        ? "Peak daily installs / UTC month"
        : `${metric === "events" ? "Events" : "Reporting installs"} per UTC ${monthly ? "month" : "day"}`;
    $("chart-empty").hidden = max > 0;
  }

  function renderTable(rows) {
    const body = $("daily-table");
    body.replaceChildren();
    for (const row of [...rows].reverse()) {
      const tr = document.createElement("tr");
      for (const value of [row.day, fmt(row.installs), fmt(row.events)]) {
        const td = document.createElement("td");
        td.textContent = value;
        tr.append(td);
      }
      body.append(tr);
    }
  }

  function renderRanked(id, rows, label, emptyText, kind = "") {
    const container = $(id);
    container.replaceChildren();
    container.dataset.kind = kind;
    const sorted = [...rows]
      .filter((row) => row.count > 0)
      .sort((a, b) => b.count - a.count || label(a).localeCompare(label(b)));
    if (!sorted.length) {
      const empty = document.createElement("div");
      empty.className = "list-empty";
      empty.textContent = emptyText;
      container.append(empty);
      return;
    }
    const largest = sorted[0].count;
    for (const row of sorted) {
      const item = document.createElement("div");
      item.className = "ranked-item";
      const top = document.createElement("div");
      top.className = "ranked-top";
      const name = document.createElement("span");
      name.className = "ranked-name";
      name.textContent = label(row);
      const count = document.createElement("span");
      count.className = "ranked-count";
      count.textContent = fmt(row.count);
      top.append(name, count);
      const track = document.createElement("div");
      track.className = "ranked-track";
      track.setAttribute("aria-hidden", "true");
      const fill = document.createElement("div");
      fill.className = "ranked-fill";
      fill.style.width = `${(row.count / largest) * 100}%`;
      track.append(fill);
      item.append(top, track);
      container.append(item);
    }
  }

  function render() {
    const report = data.ranges[range];
    const rows = rowsForRange(report);
    $("range-caption").textContent = rangeNames[range];
    $("stat-installs").textContent = fmt(report.summary.installs);
    $("stat-events").textContent = fmt(report.summary.events);
    $("stat-failures").textContent = fmt(
      report.events.find((row) => row.name === "operationFailed")?.count,
    );
    document
      .querySelectorAll("[data-range]")
      .forEach((button) =>
        button.setAttribute(
          "aria-pressed",
          String(button.dataset.range === range),
        ),
      );
    document
      .querySelectorAll("[data-metric]")
      .forEach((button) =>
        button.setAttribute(
          "aria-pressed",
          String(button.dataset.metric === metric),
        ),
      );
    renderChart(rows);
    renderTable(rows);
    renderRanked(
      "event-mix",
      report.events,
      (row) => eventNames[row.name] ?? row.name,
      "No events in this period.",
    );
    renderRanked(
      "platform-list",
      report.platforms.map((row) => ({ ...row, count: row.installs })),
      (row) => {
        const [os, arch] = row.name.split(" / ");
        return `${platformNames[os] ?? os} / ${archNames[arch] ?? arch}`;
      },
      "No platform reports yet.",
      "platform",
    );
    renderRanked(
      "version-list",
      report.versions.map((row) => ({ ...row, count: row.installs })),
      (row) => row.name,
      "No version reports yet.",
      "version",
    );
    const breakdown = (event) =>
      report.breakdowns.filter((row) => row.name === event);
    renderRanked(
      "failure-list",
      breakdown("operationFailed"),
      (row) => row.value,
      "No operation failures reported.",
    );
    renderRanked(
      "settings-list",
      breakdown("settingChanged"),
      (row) => row.value,
      "No setting changes reported.",
    );
    renderRanked(
      "session-list",
      breakdown("sessionEnded"),
      (row) => row.value,
      "No ended sessions reported.",
    );
    renderRanked(
      "library-list",
      breakdown("libraryCount"),
      (row) => row.value,
      "No library sizes reported.",
    );
    renderRanked(
      "preview-list",
      breakdown("torrentPreviewed"),
      (row) => (row.value === "magnet" ? "Magnet link" : "Torrent file"),
      "No previews reported.",
    );
    renderRanked(
      "removed-list",
      breakdown("torrentRemoved"),
      (row) => (row.value === "true" ? "Files deleted" : "Files kept"),
      "No removals reported.",
    );
    renderRanked(
      "import-list",
      breakdown("libraryImported"),
      (row) => `${row.value} torrents`,
      "No imports reported.",
    );
  }

  $("range-controls").addEventListener("click", (event) => {
    const button = event.target.closest("[data-range]");
    if (!button) return;
    range = button.dataset.range;
    render();
  });
  $("metric-controls").addEventListener("click", (event) => {
    const button = event.target.closest("[data-metric]");
    if (!button) return;
    metric = button.dataset.metric;
    render();
  });
  window.addEventListener("resize", () =>
    renderChart(rowsForRange(data.ranges[range])),
  );

  const themeButton = $("theme-button");
  const storedTheme = (() => {
    try {
      return localStorage.getItem("flume-dashboard-theme");
    } catch {
      return null;
    }
  })();
  if (storedTheme === "light" || storedTheme === "dark")
    document.documentElement.dataset.theme = storedTheme;
  function updateThemeButton() {
    const light =
      document.documentElement.dataset.theme === "light" ||
      (!document.documentElement.dataset.theme &&
        matchMedia("(prefers-color-scheme: light)").matches);
    themeButton.setAttribute(
      "aria-label",
      `Switch to ${light ? "dark" : "light"} theme`,
    );
    themeButton.title = `Switch to ${light ? "dark" : "light"} theme`;
  }
  themeButton.addEventListener("click", () => {
    const light =
      document.documentElement.dataset.theme === "light" ||
      (!document.documentElement.dataset.theme &&
        matchMedia("(prefers-color-scheme: light)").matches);
    document.documentElement.dataset.theme = light ? "dark" : "light";
    try {
      localStorage.setItem(
        "flume-dashboard-theme",
        document.documentElement.dataset.theme,
      );
    } catch {}
    updateThemeButton();
  });
  matchMedia("(prefers-color-scheme: light)").addEventListener(
    "change",
    updateThemeButton,
  );

  setStaticMeta();
  updateThemeButton();
  render();
})();
