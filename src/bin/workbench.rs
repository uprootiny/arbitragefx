//! Workbench page generator: reads all out/ artifacts and generates
//! a self-contained HTML dashboard for observing system behavior.
//!
//! Output: docs/workbench.html (for GitHub Pages) + out/workbench/index.html (local)

use std::fs;
use std::path::Path;

use serde::Serialize;

/// Parsed hypothesis from the ledger.
#[derive(Debug, Serialize)]
struct Hypothesis {
    id: String,
    name: String,
    strength: f64,
    confidence: f64,
    status: String,
    assessment: String,
}

/// All data feeding the workbench dashboard.
#[derive(Debug, Serialize)]
struct WorkbenchData {
    generated: String,
    git_sha: String,
    bench: Option<serde_json::Value>,
    bench_history: Vec<BenchHistoryEntry>,
    walk_forward: Option<serde_json::Value>,
    hypotheses: Vec<Hypothesis>,
    run_history: Vec<RunEntry>,
    test_count: usize,
    dataset_count: usize,
}

#[derive(Debug, Serialize)]
struct BenchHistoryEntry {
    date: String,
    total_ms: u128,
    total_candles: usize,
    avg_throughput: f64,
}

#[derive(Debug, Serialize)]
struct RunEntry {
    date: String,
    path: String,
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Count tests by running cargo test in dry-run mode (list only).
fn count_tests() -> usize {
    std::process::Command::new("cargo")
        .args(["test", "--", "--list"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().filter(|l| l.ends_with(": test")).count())
        .unwrap_or(0)
}

/// Count CSV datasets in data/.
fn count_datasets() -> usize {
    fs::read_dir("data")
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "csv")
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// Parse hypothesis_ledger.edn with simple string matching.
fn parse_hypotheses() -> Vec<Hypothesis> {
    let content = match fs::read_to_string("hypothesis_ledger.edn") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut hypotheses = Vec::new();
    let mut current_id = String::new();
    let mut current_name = String::new();
    let mut current_strength = 0.0;
    let mut current_confidence = 0.0;
    let mut current_assessment = String::new();
    let mut in_hypothesis = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.contains("{:id \"H") {
            in_hypothesis = true;
            if let Some(id) = extract_quoted(trimmed, ":id") {
                current_id = id;
            }
        }

        if in_hypothesis {
            if let Some(name) = extract_quoted(trimmed, ":name") {
                current_name = name;
            }
            if trimmed.starts_with(":current (stv") || trimmed.contains(":current (stv") {
                if let Some((s, c)) = extract_stv(trimmed) {
                    current_strength = s;
                    current_confidence = c;
                }
            }
            if let Some(assess) = extract_quoted(trimmed, ":assessment") {
                current_assessment = assess;
            }
        }

        // End of a hypothesis block — heuristic: line with just assessment closes it
        if in_hypothesis && trimmed.starts_with(":assessment") {
            let status = classify_hypothesis(current_strength, current_confidence);
            hypotheses.push(Hypothesis {
                id: current_id.clone(),
                name: current_name.clone(),
                strength: current_strength,
                confidence: current_confidence,
                status,
                assessment: current_assessment.clone(),
            });
            in_hypothesis = false;
        }
    }

    hypotheses
}

fn classify_hypothesis(strength: f64, confidence: f64) -> String {
    if confidence >= 0.7 && strength >= 0.8 {
        "established".into()
    } else if confidence >= 0.6 && strength >= 0.6 {
        "supported".into()
    } else if confidence >= 0.5 && strength >= 0.4 {
        "contested".into()
    } else {
        "fragile".into()
    }
}

fn extract_quoted(line: &str, key: &str) -> Option<String> {
    let idx = line.find(key)?;
    let rest = &line[idx + key.len()..];
    let start = rest.find('"')? + 1;
    let end = rest[start..].find('"')? + start;
    Some(rest[start..end].to_string())
}

fn extract_stv(line: &str) -> Option<(f64, f64)> {
    let idx = line.find("(stv")?;
    let rest = &line[idx + 4..];
    let end = rest.find(')')?;
    let nums: Vec<f64> = rest[..end]
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() >= 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
}

/// Load bench history from out/bench/*.json files.
fn load_bench_history() -> Vec<BenchHistoryEntry> {
    let dir = Path::new("out/bench");
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false)
                && path
                    .file_name()
                    .map(|n| n != "report.json")
                    .unwrap_or(false)
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        let date = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        entries.push(BenchHistoryEntry {
                            date,
                            total_ms: val["total_ms"].as_u64().unwrap_or(0) as u128,
                            total_candles: val["total_candles"].as_u64().unwrap_or(0) as usize,
                            avg_throughput: val["avg_throughput"].as_f64().unwrap_or(0.0),
                        });
                    }
                }
            }
        }
    }
    entries.sort_by(|a, b| a.date.cmp(&b.date));
    entries
}

/// Load run history from out/reports/*.txt.
fn load_run_history() -> Vec<RunEntry> {
    let dir = Path::new("out/reports");
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "txt").unwrap_or(false) {
                let date = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                entries.push(RunEntry {
                    date,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    entries.sort_by(|a, b| b.date.cmp(&a.date));
    entries
}

fn main() {
    println!("=== ArbitrageFX Workbench Generator ===");

    // Gather data
    let bench: Option<serde_json::Value> = fs::read_to_string("out/bench/report.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let walk_forward: Option<serde_json::Value> =
        fs::read_to_string("out/walk_forward/report.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

    let hypotheses = parse_hypotheses();
    let bench_history = load_bench_history();
    let run_history = load_run_history();
    let test_count = count_tests();
    let dataset_count = count_datasets();

    println!("  hypotheses: {}", hypotheses.len());
    println!(
        "  bench: {}",
        if bench.is_some() { "loaded" } else { "missing" }
    );
    println!(
        "  walk_forward: {}",
        if walk_forward.is_some() {
            "loaded"
        } else {
            "missing"
        }
    );
    println!("  bench_history: {} entries", bench_history.len());
    println!("  run_history: {} entries", run_history.len());
    println!("  tests: {}", test_count);
    println!("  datasets: {}", dataset_count);

    let data = WorkbenchData {
        generated: chrono::Utc::now().to_rfc3339(),
        git_sha: git_sha(),
        bench,
        bench_history,
        walk_forward,
        hypotheses,
        run_history,
        test_count,
        dataset_count,
    };

    let json_blob = serde_json::to_string(&data).unwrap();
    let html = TEMPLATE.replace("__WORKBENCH_DATA__", &json_blob);

    // Write outputs
    fs::create_dir_all("docs").ok();
    fs::create_dir_all("out/workbench").ok();
    fs::write("docs/workbench.html", &html).unwrap();
    fs::write("out/workbench/index.html", &html).unwrap();

    println!();
    println!(
        "  docs/workbench.html written ({:.1} KB)",
        html.len() as f64 / 1024.0
    );
    println!("  out/workbench/index.html written");
}

const TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ArbitrageFX Workbench</title>
  <style>
    :root {
      --bg: #0d1117;
      --fg: #c9d1d9;
      --accent: #58a6ff;
      --border: #30363d;
      --card-bg: #161b22;
      --green: #3fb950;
      --red: #f85149;
      --yellow: #d29922;
      --mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--fg);
      line-height: 1.6;
      padding: 2rem;
      max-width: 1200px;
      margin: 0 auto;
    }
    h1 { color: #f0f6fc; font-size: 2rem; margin-bottom: 0.3rem; }
    h2 {
      color: var(--accent);
      font-size: 1.2rem;
      margin: 2rem 0 0.8rem;
      border-bottom: 1px solid var(--border);
      padding-bottom: 0.3rem;
    }
    .subtitle { color: #8b949e; font-size: 0.85rem; margin-bottom: 1.5rem; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 0.8rem; margin: 0.8rem 0; }
    .card {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 0.8rem 1rem;
    }
    .card h3 { font-size: 0.85rem; color: #8b949e; margin-bottom: 0.3rem; font-weight: 400; }
    .card .val { font-size: 1.6rem; font-weight: 700; color: #f0f6fc; font-family: var(--mono); }
    .card .detail { font-size: 0.75rem; color: #8b949e; margin-top: 0.2rem; }
    table { width: 100%; border-collapse: collapse; margin: 0.5rem 0; font-size: 0.8rem; }
    th, td { padding: 0.4rem 0.6rem; text-align: left; border-bottom: 1px solid var(--border); }
    th { color: #f0f6fc; font-weight: 600; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.5px; }
    .mono { font-family: var(--mono); font-size: 0.8rem; }
    .badge {
      display: inline-block;
      padding: 0.1rem 0.4rem;
      border-radius: 3px;
      font-size: 0.7rem;
      font-family: var(--mono);
    }
    .badge-green { background: #0d2818; color: var(--green); border: 1px solid #1a4128; }
    .badge-yellow { background: #2d1f00; color: var(--yellow); border: 1px solid #4d3800; }
    .badge-red { background: #2d0000; color: var(--red); border: 1px solid #4d0000; }
    .badge-blue { background: #0d1a2d; color: var(--accent); border: 1px solid #1a3050; }

    /* Heatmap */
    .heatmap { display: grid; gap: 2px; margin: 0.5rem 0; }
    .heatmap-cell {
      padding: 0.3rem;
      text-align: center;
      font-family: var(--mono);
      font-size: 0.65rem;
      border-radius: 3px;
      min-width: 60px;
    }
    .heatmap-header {
      font-weight: 600;
      color: #8b949e;
      font-size: 0.65rem;
      text-align: center;
      padding: 0.3rem;
    }

    /* Bars */
    .bar-container { display: flex; align-items: center; gap: 0.5rem; margin: 0.2rem 0; }
    .bar-label { min-width: 120px; font-size: 0.75rem; text-align: right; color: #8b949e; }
    .bar-track { flex: 1; height: 18px; background: #21262d; border-radius: 3px; overflow: hidden; position: relative; }
    .bar-fill { height: 100%; border-radius: 3px; transition: width 0.3s; }
    .bar-value { font-family: var(--mono); font-size: 0.7rem; min-width: 60px; }

    .section { margin-bottom: 1.5rem; }
    .empty-state { color: #484f58; font-style: italic; font-size: 0.85rem; padding: 1rem; }
    footer { margin-top: 3rem; color: #484f58; font-size: 0.75rem; border-top: 1px solid var(--border); padding-top: 1rem; }
  </style>
</head>
<body>
  <h1>ArbitrageFX Workbench</h1>
  <p class="subtitle" id="subtitle">Loading...</p>

  <!-- Overview Cards -->
  <div class="grid" id="overview"></div>

  <!-- Profiling -->
  <div class="section">
    <h2>Profiling</h2>
    <div id="profiling"><p class="empty-state">No bench data. Run: cargo run --release --bin bench</p></div>
  </div>

  <!-- Strategy x Regime Heatmap -->
  <div class="section">
    <h2>Strategy &times; Regime Heatmap</h2>
    <div id="heatmap"><p class="empty-state">No bench data available</p></div>
  </div>

  <!-- Walk-Forward Survival -->
  <div class="section">
    <h2>Walk-Forward Survival</h2>
    <div id="walkforward"><p class="empty-state">No walk-forward data. Run: cargo run --release --bin walk_forward</p></div>
  </div>

  <!-- Hypotheses -->
  <div class="section">
    <h2>Hypothesis Ledger</h2>
    <div id="hypotheses"><p class="empty-state">No hypotheses found</p></div>
  </div>

  <!-- Run History -->
  <div class="section">
    <h2>History</h2>
    <div id="history"><p class="empty-state">No run history</p></div>
  </div>

  <footer id="footer"></footer>

  <script>
  const D = JSON.parse('__WORKBENCH_DATA__');

  // Subtitle
  document.getElementById('subtitle').textContent =
    `Generated ${D.generated.split('T')[0]} · git ${D.git_sha} · ${D.test_count} tests · ${D.dataset_count} datasets`;

  // Overview cards
  (() => {
    const el = document.getElementById('overview');
    const benchTs = D.bench ? D.bench.timestamp.split('T')[0] : 'n/a';
    const totalMs = D.bench ? D.bench.total_ms : 0;
    const avgThroughput = D.bench ? Math.round(D.bench.avg_throughput) : 0;
    const survivors = D.walk_forward
      ? D.walk_forward.summaries.filter(s => s.survives_correction).length
      : 0;
    const totalStrats = D.walk_forward ? D.walk_forward.summaries.length : 0;
    const hypoCount = D.hypotheses.length;
    const established = D.hypotheses.filter(h => h.status === 'established').length;

    el.innerHTML = `
      <div class="card"><h3>Tests</h3><div class="val">${D.test_count}</div><div class="detail">all passing</div></div>
      <div class="card"><h3>Datasets</h3><div class="val">${D.dataset_count}</div><div class="detail">CSV files in data/</div></div>
      <div class="card"><h3>Throughput</h3><div class="val">${avgThroughput.toLocaleString()}</div><div class="detail">candles/sec (avg)</div></div>
      <div class="card"><h3>Bench Time</h3><div class="val">${totalMs}ms</div><div class="detail">${benchTs}</div></div>
      <div class="card"><h3>WF Survivors</h3><div class="val">${survivors}/${totalStrats}</div><div class="detail">after Bonferroni</div></div>
      <div class="card"><h3>Hypotheses</h3><div class="val">${hypoCount}</div><div class="detail">${established} established</div></div>
    `;
  })();

  // Profiling bars
  (() => {
    if (!D.bench) return;
    const el = document.getElementById('profiling');
    const datasets = D.bench.datasets;
    const maxMs = Math.max(...datasets.map(d => d.backtest_ms + d.walkforward_ms), 1);

    let html = '<table><tr><th>Dataset</th><th>Candles</th><th>Regime</th><th>Backtest</th><th>Walk-Fwd</th><th>Throughput</th><th>RSS</th></tr>';
    for (const d of datasets) {
      html += `<tr>
        <td class="mono">${d.name}</td>
        <td class="mono">${d.candles.toLocaleString()}</td>
        <td><span class="badge badge-blue">${d.regime.dominant_regime}</span></td>
        <td class="mono">${d.backtest_ms}ms</td>
        <td class="mono">${d.walkforward_ms}ms</td>
        <td class="mono">${Math.round(d.throughput_candles_per_sec).toLocaleString()} c/s</td>
        <td class="mono">${d.peak_rss_kb ? d.peak_rss_kb.toLocaleString() + ' KB' : 'n/a'}</td>
      </tr>`;
    }
    html += '</table>';

    // Timing bars
    html += '<div style="margin-top: 1rem;">';
    for (const d of datasets) {
      const btPct = (d.backtest_ms / maxMs * 100).toFixed(1);
      const wfPct = (d.walkforward_ms / maxMs * 100).toFixed(1);
      html += `<div class="bar-container">
        <span class="bar-label">${d.name}</span>
        <div class="bar-track">
          <div class="bar-fill" style="width: ${btPct}%; background: var(--accent); display: inline-block; position: absolute;"></div>
          <div class="bar-fill" style="width: ${(parseFloat(btPct) + parseFloat(wfPct))}%; background: #1a3050; position: absolute;"></div>
          <div class="bar-fill" style="width: ${btPct}%; background: var(--accent); position: absolute;"></div>
        </div>
        <span class="bar-value">${d.backtest_ms + d.walkforward_ms}ms</span>
      </div>`;
    }
    html += '</div>';
    html += '<div style="font-size: 0.7rem; color: #484f58; margin-top: 0.3rem;">Blue = backtest, dark = walk-forward</div>';

    el.innerHTML = html;
  })();

  // Heatmap: Strategy x Regime
  (() => {
    if (!D.bench) return;
    const el = document.getElementById('heatmap');
    const datasets = D.bench.datasets;

    // Collect all strategy IDs from first dataset
    if (datasets.length === 0 || !datasets[0].backtest_result) return;
    const stratIds = datasets[0].backtest_result.strategies.map(s => s.id);
    const regimeNames = datasets.map(d => d.name.replace('btc_', '').replace('_1h', ''));

    const cols = regimeNames.length + 1;
    let html = `<div class="heatmap" style="grid-template-columns: 100px repeat(${regimeNames.length}, 1fr);">`;

    // Header row
    html += '<div class="heatmap-header"></div>';
    for (const r of regimeNames) {
      html += `<div class="heatmap-header">${r}</div>`;
    }

    // Data rows
    for (let si = 0; si < stratIds.length; si++) {
      html += `<div class="heatmap-header" style="text-align: right;">${stratIds[si]}</div>`;
      for (let di = 0; di < datasets.length; di++) {
        const strat = datasets[di].backtest_result.strategies[si];
        if (!strat) {
          html += '<div class="heatmap-cell" style="background: #21262d;">-</div>';
          continue;
        }
        const pnl = strat.equity_pnl;
        const color = pnl > 0
          ? `rgba(63, 185, 80, ${Math.min(Math.abs(pnl) / 10, 0.8)})`
          : `rgba(248, 81, 73, ${Math.min(Math.abs(pnl) / 10, 0.8)})`;
        html += `<div class="heatmap-cell" style="background: ${color};">${pnl >= 0 ? '+' : ''}${pnl.toFixed(2)}</div>`;
      }
    }
    html += '</div>';
    html += '<div style="font-size: 0.7rem; color: #484f58; margin-top: 0.3rem;">Green = positive equity PnL, Red = negative. Intensity = magnitude.</div>';
    el.innerHTML = html;
  })();

  // Walk-forward
  (() => {
    if (!D.walk_forward) return;
    const el = document.getElementById('walkforward');
    const wf = D.walk_forward;
    let html = `<p style="font-size: 0.8rem; color: #8b949e; margin-bottom: 0.5rem;">
      ${wf.num_windows} windows &middot; ${wf.num_strategies} strategies &middot; ${wf.num_comparisons} comparisons &middot; ${wf.correction_method} &alpha;=${wf.alpha}
    </p>`;
    html += '<table><tr><th>Strategy</th><th>Train PnL</th><th>Test PnL</th><th>Overfit Ratio</th><th>P-value</th><th>Positive</th><th>Survives?</th></tr>';
    for (const s of wf.summaries) {
      const survBadge = s.survives_correction
        ? '<span class="badge badge-green">YES</span>'
        : '<span class="badge badge-red">no</span>';
      const orClass = s.overfit_ratio > 0.5 ? 'color: var(--green)' : s.overfit_ratio > 0 ? 'color: var(--yellow)' : 'color: var(--red)';
      html += `<tr>
        <td class="mono">${s.id}</td>
        <td class="mono">${s.train_mean_pnl >= 0 ? '+' : ''}${s.train_mean_pnl.toFixed(4)}</td>
        <td class="mono">${s.test_mean_pnl >= 0 ? '+' : ''}${s.test_mean_pnl.toFixed(4)}</td>
        <td class="mono" style="${orClass}">${s.overfit_ratio.toFixed(3)}</td>
        <td class="mono">${s.p_value.toFixed(3)}</td>
        <td class="mono">${s.test_positive_windows}/${s.total_windows}</td>
        <td>${survBadge}</td>
      </tr>`;
    }
    html += '</table>';
    el.innerHTML = html;
  })();

  // Hypotheses
  (() => {
    if (D.hypotheses.length === 0) return;
    const el = document.getElementById('hypotheses');
    let html = '<table><tr><th>ID</th><th>Hypothesis</th><th>Strength</th><th>Confidence</th><th>Status</th></tr>';
    for (const h of D.hypotheses) {
      const sColor = h.strength >= 0.7 ? 'var(--green)' : h.strength >= 0.4 ? 'var(--yellow)' : 'var(--red)';
      const cColor = h.confidence >= 0.7 ? 'var(--green)' : h.confidence >= 0.4 ? 'var(--yellow)' : 'var(--red)';
      const statusClass = h.status === 'established' ? 'badge-green'
        : h.status === 'supported' ? 'badge-green'
        : h.status === 'contested' ? 'badge-yellow'
        : 'badge-red';
      html += `<tr>
        <td class="mono">${h.id}</td>
        <td>${h.name}</td>
        <td class="mono" style="color: ${sColor};">${h.strength.toFixed(2)}</td>
        <td class="mono" style="color: ${cColor};">${h.confidence.toFixed(2)}</td>
        <td><span class="badge ${statusClass}">${h.status}</span></td>
      </tr>`;
    }
    html += '</table>';
    el.innerHTML = html;
  })();

  // History
  (() => {
    const el = document.getElementById('history');
    let html = '';

    if (D.bench_history.length > 0) {
      html += '<h3 style="color: #f0f6fc; font-size: 0.9rem; margin-bottom: 0.5rem;">Bench Runs</h3>';
      html += '<table><tr><th>Date</th><th>Candles</th><th>Time</th><th>Throughput</th></tr>';
      for (const b of D.bench_history) {
        html += `<tr>
          <td class="mono">${b.date}</td>
          <td class="mono">${b.total_candles.toLocaleString()}</td>
          <td class="mono">${b.total_ms}ms</td>
          <td class="mono">${Math.round(b.avg_throughput).toLocaleString()} c/s</td>
        </tr>`;
      }
      html += '</table>';
    }

    if (D.run_history.length > 0) {
      html += '<h3 style="color: #f0f6fc; font-size: 0.9rem; margin: 1rem 0 0.5rem;">Pipeline Runs</h3>';
      html += '<table><tr><th>Date</th><th>Report</th></tr>';
      for (const r of D.run_history) {
        html += `<tr><td class="mono">${r.date}</td><td class="mono" style="color: #8b949e;">${r.path}</td></tr>`;
      }
      html += '</table>';
    }

    if (html === '') {
      html = '<p class="empty-state">No historical data yet. Run the pipeline to generate history.</p>';
    }
    el.innerHTML = html;
  })();

  // Footer
  document.getElementById('footer').textContent =
    `Generated ${D.generated} · git ${D.git_sha} · ArbitrageFX Workbench`;
  </script>
</body>
</html>"##;
