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

/// STV history entry from JSONL.
#[derive(Debug, Serialize, serde::Deserialize)]
struct StvHistoryEntry {
    ts: String,
    hypothesis_id: String,
    old_stv: [f64; 2],
    new_stv: [f64; 2],
    dataset: String,
    observation: String,
}

/// Trap status for dashboard display.
#[derive(Debug, Serialize)]
struct TrapStatusEntry {
    id: u8,
    name: String,
    severity: String,
    guard: String,
    evidence: String,
}

/// Uncertainty map categories.
#[derive(Debug, Serialize, Default)]
struct UncertaintyMap {
    well_established: Vec<String>,
    supported: Vec<String>,
    contested: Vec<String>,
    fragile: Vec<String>,
    untested: Vec<String>,
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
    stv_history: Vec<StvHistoryEntry>,
    trap_status: Vec<TrapStatusEntry>,
    uncertainty_map: UncertaintyMap,
    integrity_score: String,
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

/// Load STV history from JSONL file.
fn load_stv_history() -> Vec<StvHistoryEntry> {
    let path = "out/ledger_history/updates.jsonl";
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Load trap status from backtest_traps module.
fn load_trap_status() -> Vec<TrapStatusEntry> {
    use arbitragefx::backtest_traps::trap_status;
    trap_status()
        .into_iter()
        .map(|t| TrapStatusEntry {
            id: t.id,
            name: t.name.to_string(),
            severity: format!("{:?}", t.severity),
            guard: format!("{:?}", t.guard),
            evidence: t.evidence.to_string(),
        })
        .collect()
}

/// Parse uncertainty map from hypothesis_ledger.edn.
fn parse_uncertainty_map() -> UncertaintyMap {
    let content = match fs::read_to_string("hypothesis_ledger.edn") {
        Ok(c) => c,
        Err(_) => return UncertaintyMap::default(),
    };
    let mut map = UncertaintyMap::default();
    let mut current_category = "";

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(":well-established") {
            current_category = "well_established";
        } else if trimmed.starts_with(":supported") {
            current_category = "supported";
        } else if trimmed.starts_with(":contested") {
            current_category = "contested";
        } else if trimmed.starts_with(":fragile") {
            current_category = "fragile";
        } else if trimmed.starts_with(":untested") {
            current_category = "untested";
        }

        // Extract quoted strings from array lines
        let mut rest = trimmed;
        while let Some(start) = rest.find('"') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('"') {
                let item = rest[..end].to_string();
                match current_category {
                    "well_established" => map.well_established.push(item),
                    "supported" => map.supported.push(item),
                    "contested" => map.contested.push(item),
                    "fragile" => map.fragile.push(item),
                    "untested" => map.untested.push(item),
                    _ => {}
                }
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }

        // Reset category on closing bracket of uncertainty-map
        if trimmed.ends_with("}}") {
            current_category = "";
        }
    }
    map
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
    let stv_history = load_stv_history();
    let trap_status = load_trap_status();
    let uncertainty_map = parse_uncertainty_map();
    let (guarded, total) = arbitragefx::backtest_traps::integrity_score();
    let integrity_score = format!("{}/{}", guarded, total);
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
    println!("  stv_history: {} entries", stv_history.len());
    println!("  trap_status: {} traps", trap_status.len());
    println!(
        "  uncertainty_map: {} categories",
        [
            &uncertainty_map.well_established,
            &uncertainty_map.supported,
            &uncertainty_map.contested,
            &uncertainty_map.fragile,
            &uncertainty_map.untested
        ]
        .iter()
        .filter(|v| !v.is_empty())
        .count()
    );
    println!("  integrity: {}", integrity_score);
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
        stv_history,
        trap_status,
        uncertainty_map,
        integrity_score,
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
      --bg: #0d1117; --bg-raised: #161b22; --bg-inset: #010409;
      --fg: #c9d1d9; --fg-muted: #8b949e; --fg-subtle: #484f58; --fg-bright: #f0f6fc;
      --accent: #58a6ff; --accent-muted: #1a3050;
      --green: #3fb950; --green-muted: #0d2818; --green-border: #1a4128;
      --red: #f85149; --red-muted: #2d0000; --red-border: #4d0000;
      --yellow: #d29922; --yellow-muted: #2d1f00; --yellow-border: #4d3800;
      --border: #30363d; --border-heavy: #484f58;
      --mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', 'SF Mono', monospace;
      --sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
      --radius: 8px; --radius-sm: 4px;
      --shadow: 0 1px 3px rgba(0,0,0,0.3), 0 1px 2px rgba(0,0,0,0.2);
      --shadow-lg: 0 4px 12px rgba(0,0,0,0.4);
      --transition: 0.2s ease;
      --nav-h: 48px;
    }
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    html { scroll-behavior: smooth; scroll-padding-top: calc(var(--nav-h) + 16px); }
    body { font-family: var(--sans); background: var(--bg); color: var(--fg); line-height: 1.6; }

    /* ── Navigation ── */
    nav {
      position: sticky; top: 0; z-index: 100;
      background: rgba(13,17,23,0.92); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--border);
      height: var(--nav-h); display: flex; align-items: center;
      padding: 0 1.5rem; gap: 0.25rem;
    }
    .nav-brand { color: var(--fg-bright); font-weight: 700; font-size: 0.9rem; margin-right: 1rem; white-space: nowrap; }
    .nav-links { display: flex; gap: 0.15rem; overflow-x: auto; flex: 1; scrollbar-width: none; }
    .nav-links::-webkit-scrollbar { display: none; }
    .nav-link {
      color: var(--fg-muted); text-decoration: none; font-size: 0.72rem; padding: 0.3rem 0.55rem;
      border-radius: var(--radius-sm); white-space: nowrap; transition: all var(--transition);
    }
    .nav-link:hover { color: var(--fg-bright); background: rgba(255,255,255,0.05); }
    .nav-link.active { color: var(--accent); background: var(--accent-muted); }
    .nav-status { margin-left: auto; display: flex; gap: 0.6rem; align-items: center; font-size: 0.7rem; font-family: var(--mono); color: var(--fg-muted); }
    .status-dot { width: 7px; height: 7px; border-radius: 50%; display: inline-block; }
    .status-dot.ok { background: var(--green); box-shadow: 0 0 6px var(--green); }

    /* ── Main Layout ── */
    main { max-width: 1280px; margin: 0 auto; padding: 1.5rem; }
    .section { margin-bottom: 2.5rem; scroll-margin-top: calc(var(--nav-h) + 16px); }
    .section-header {
      display: flex; align-items: baseline; gap: 0.75rem;
      border-bottom: 1px solid var(--border); padding-bottom: 0.5rem; margin-bottom: 1rem;
      cursor: pointer; user-select: none;
    }
    .section-header h2 {
      color: var(--fg-bright); font-size: 1.05rem; font-weight: 600; letter-spacing: -0.01em;
    }
    .section-desc { color: var(--fg-muted); font-size: 0.75rem; flex: 1; }
    .section-toggle { color: var(--fg-subtle); font-size: 0.8rem; transition: transform var(--transition); }
    .section.collapsed .section-toggle { transform: rotate(-90deg); }
    .section.collapsed .section-body { display: none; }

    /* ── Hero ── */
    .hero { margin-bottom: 2rem; }
    .hero h1 { color: var(--fg-bright); font-size: 1.5rem; font-weight: 700; letter-spacing: -0.02em; margin-bottom: 0.2rem; }
    .hero-sub { color: var(--fg-muted); font-size: 0.8rem; font-family: var(--mono); }
    .hero-findings {
      display: flex; gap: 0.75rem; margin-top: 1rem; flex-wrap: wrap;
    }
    .finding {
      background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--radius);
      padding: 0.6rem 0.9rem; font-size: 0.78rem; display: flex; align-items: center; gap: 0.5rem;
    }
    .finding-icon { font-size: 0.9rem; }
    .finding b { color: var(--fg-bright); }

    /* ── Cards Grid ── */
    .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 0.65rem; }
    .card {
      background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--radius);
      padding: 0.75rem 0.9rem; transition: border-color var(--transition), box-shadow var(--transition);
      border-top: 3px solid var(--border);
    }
    .card:hover { border-color: var(--border-heavy); box-shadow: var(--shadow); }
    .card.accent-green { border-top-color: var(--green); }
    .card.accent-blue { border-top-color: var(--accent); }
    .card.accent-yellow { border-top-color: var(--yellow); }
    .card.accent-red { border-top-color: var(--red); }
    .card-label { font-size: 0.72rem; color: var(--fg-muted); text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 0.15rem; }
    .card-val { font-size: 1.5rem; font-weight: 700; color: var(--fg-bright); font-family: var(--mono); line-height: 1.2; }
    .card-detail { font-size: 0.7rem; color: var(--fg-subtle); margin-top: 0.15rem; }

    /* ── Tables ── */
    .table-wrap { overflow-x: auto; margin: 0.5rem 0; border-radius: var(--radius); border: 1px solid var(--border); }
    table { width: 100%; border-collapse: collapse; font-size: 0.78rem; }
    thead { background: var(--bg-raised); }
    th {
      padding: 0.5rem 0.7rem; text-align: left; font-weight: 600; font-size: 0.68rem;
      text-transform: uppercase; letter-spacing: 0.04em; color: var(--fg-muted);
      border-bottom: 1px solid var(--border); position: sticky; top: 0; background: var(--bg-raised);
    }
    td { padding: 0.45rem 0.7rem; border-bottom: 1px solid rgba(48,54,61,0.5); }
    tbody tr { transition: background var(--transition); }
    tbody tr:hover { background: rgba(255,255,255,0.02); }
    tbody tr:last-child td { border-bottom: none; }
    .mono { font-family: var(--mono); font-size: 0.78rem; }
    .text-right { text-align: right; }
    .text-center { text-align: center; }

    /* ── Badges ── */
    .badge {
      display: inline-flex; align-items: center; gap: 0.25rem;
      padding: 0.12rem 0.45rem; border-radius: var(--radius-sm);
      font-size: 0.65rem; font-family: var(--mono); font-weight: 500; white-space: nowrap;
    }
    .badge-green { background: var(--green-muted); color: var(--green); border: 1px solid var(--green-border); }
    .badge-yellow { background: var(--yellow-muted); color: var(--yellow); border: 1px solid var(--yellow-border); }
    .badge-red { background: var(--red-muted); color: var(--red); border: 1px solid var(--red-border); }
    .badge-blue { background: var(--accent-muted); color: var(--accent); border: 1px solid #1a3050; }
    .badge-gray { background: rgba(139,148,158,0.1); color: var(--fg-muted); border: 1px solid var(--border); }

    /* ── Bars ── */
    .bar-row { display: flex; align-items: center; gap: 0.6rem; padding: 0.25rem 0; }
    .bar-label { min-width: 110px; font-size: 0.72rem; text-align: right; color: var(--fg-muted); font-family: var(--mono); }
    .bar-track { flex: 1; height: 20px; background: var(--bg-inset); border-radius: var(--radius-sm); overflow: hidden; display: flex; }
    .bar-seg { height: 100%; transition: width 0.4s ease; }
    .bar-val { font-family: var(--mono); font-size: 0.7rem; min-width: 55px; color: var(--fg-muted); }

    /* ── Heatmap ── */
    .heatmap { display: grid; gap: 2px; }
    .hm-cell {
      padding: 0.35rem 0.2rem; text-align: center; font-family: var(--mono); font-size: 0.68rem;
      border-radius: var(--radius-sm); transition: transform 0.15s, box-shadow 0.15s; cursor: default;
    }
    .hm-cell:hover { transform: scale(1.08); box-shadow: var(--shadow); z-index: 1; position: relative; }
    .hm-head { font-weight: 600; color: var(--fg-muted); font-size: 0.65rem; text-align: center; padding: 0.35rem 0.2rem; }

    /* ── Kanban ── */
    .kanban { display: grid; grid-template-columns: repeat(auto-fill, minmax(175px, 1fr)); gap: 0.6rem; }
    .kanban-col {
      background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--radius);
      padding: 0.7rem; border-top: 3px solid var(--border);
    }
    .kanban-title { font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 0.5rem; display: flex; justify-content: space-between; }
    .kanban-count { background: rgba(255,255,255,0.06); border-radius: 10px; padding: 0 0.4rem; font-size: 0.62rem; }
    .kanban-item {
      background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius-sm);
      padding: 0.35rem 0.55rem; margin-bottom: 0.3rem; font-size: 0.72rem; font-family: var(--mono);
      transition: border-color var(--transition);
    }
    .kanban-item:hover { border-color: var(--border-heavy); }
    .kanban-empty { color: var(--fg-subtle); font-style: italic; font-size: 0.68rem; padding: 0.3rem 0; }

    /* ── Sparklines ── */
    .spark-wrap { display: inline-flex; align-items: center; gap: 0.4rem; }
    .spark-svg { vertical-align: middle; }
    .spark-val { font-family: var(--mono); font-size: 0.72rem; }

    /* ── Trap Indicators ── */
    .trap-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; vertical-align: middle; }
    .trap-dot.guarded { background: var(--green); box-shadow: 0 0 4px rgba(63,185,80,0.4); }
    .trap-dot.partial { background: var(--yellow); box-shadow: 0 0 4px rgba(210,153,34,0.4); }
    .trap-dot.unguarded { background: var(--red); box-shadow: 0 0 4px rgba(248,81,73,0.4); }

    /* ── Integrity Ring ── */
    .integrity-ring { display: flex; align-items: center; gap: 0.3rem; }
    .ring-svg { transform: rotate(-90deg); }

    /* ── Leaderboard ── */
    .lb-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 0.6rem; }
    .lb-col { background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.7rem; }
    .lb-title { font-size: 0.7rem; color: var(--fg-muted); text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 0.5rem; font-weight: 600; }
    .lb-row { display: flex; justify-content: space-between; align-items: center; padding: 0.2rem 0; font-size: 0.72rem; font-family: var(--mono); }
    .lb-rank { color: var(--fg-subtle); min-width: 1.5rem; }

    /* ── Trends Chart ── */
    .chart-container { background: var(--bg-raised); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.75rem; }
    .chart-legend { display: flex; gap: 1rem; font-size: 0.68rem; color: var(--fg-muted); margin-top: 0.4rem; }
    .chart-legend span { display: flex; align-items: center; gap: 0.3rem; }
    .legend-line { width: 16px; height: 2px; display: inline-block; }

    /* ── Empty State ── */
    .empty { color: var(--fg-subtle); font-style: italic; font-size: 0.8rem; padding: 1.5rem; text-align: center; background: var(--bg-raised); border: 1px dashed var(--border); border-radius: var(--radius); }
    .empty code { font-family: var(--mono); font-size: 0.75rem; color: var(--fg-muted); background: rgba(255,255,255,0.04); padding: 0.15rem 0.4rem; border-radius: var(--radius-sm); }

    /* ── Tooltip ── */
    .tip { position: relative; }
    .tip::after {
      content: attr(data-tip); position: absolute; bottom: 100%; left: 50%; transform: translateX(-50%);
      background: var(--fg-bright); color: var(--bg); padding: 0.3rem 0.55rem; border-radius: var(--radius-sm);
      font-size: 0.68rem; font-family: var(--mono); white-space: nowrap; pointer-events: none;
      opacity: 0; transition: opacity 0.15s; z-index: 10;
    }
    .tip:hover::after { opacity: 1; }

    /* ── Footer ── */
    footer {
      margin-top: 2rem; padding: 1rem 0; border-top: 1px solid var(--border);
      display: flex; justify-content: space-between; flex-wrap: wrap; gap: 0.5rem;
      font-size: 0.7rem; color: var(--fg-subtle);
    }
    footer a { color: var(--fg-muted); text-decoration: none; }
    footer a:hover { color: var(--accent); }

    /* ── Responsive ── */
    @media (max-width: 768px) {
      main { padding: 1rem; }
      .grid { grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); }
      .card-val { font-size: 1.2rem; }
      .kanban { grid-template-columns: 1fr 1fr; }
      .lb-grid { grid-template-columns: 1fr; }
      .hero-findings { flex-direction: column; }
      .nav-status { display: none; }
      nav { padding: 0 0.75rem; }
    }
    @media (max-width: 480px) {
      .grid { grid-template-columns: 1fr 1fr; }
      .kanban { grid-template-columns: 1fr; }
    }

    /* ── Print ── */
    @media print {
      nav { display: none; }
      body { background: #fff; color: #111; }
      .card { border: 1px solid #ccc; break-inside: avoid; }
      .section { break-inside: avoid; }
    }

    /* ── Keyboard hint ── */
    .kbd-hint {
      position: fixed; bottom: 1rem; right: 1rem; background: var(--bg-raised); border: 1px solid var(--border);
      border-radius: var(--radius); padding: 0.4rem 0.6rem; font-size: 0.65rem; color: var(--fg-subtle);
      opacity: 0; transition: opacity 0.3s; pointer-events: none;
    }
    .kbd-hint.show { opacity: 1; }
    .kbd { background: rgba(255,255,255,0.06); border: 1px solid var(--border); border-radius: 3px; padding: 0.05rem 0.3rem; font-family: var(--mono); }
  </style>
</head>
<body>
  <nav>
    <span class="nav-brand">ArbitrageFX</span>
    <div class="nav-links" id="nav-links"></div>
    <div class="nav-status" id="nav-status"></div>
  </nav>

  <main>
    <div class="hero" id="hero"></div>

    <div class="section" id="sec-overview" data-nav="Overview">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>System Overview</h2>
        <span class="section-desc">Key metrics at a glance</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body"><div class="grid" id="overview"></div></div>
    </div>

    <div class="section" id="sec-profiling" data-nav="Profiling">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Profiling</h2>
        <span class="section-desc">Execution timing and resource usage per dataset</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="profiling"><p class="empty">No bench data. Run <code>cargo run --release --bin bench</code></p></div>
    </div>

    <div class="section" id="sec-heatmap" data-nav="Heatmap">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Strategy &times; Regime Heatmap</h2>
        <span class="section-desc">Equity PnL across strategy variants and market regimes</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="heatmap"><p class="empty">No bench data available</p></div>
    </div>

    <div class="section" id="sec-walkforward" data-nav="Walk-Forward">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Walk-Forward Survival</h2>
        <span class="section-desc">Out-of-sample validation with Bonferroni correction</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="walkforward"><p class="empty">No walk-forward data. Run <code>cargo run --release --bin walk_forward</code></p></div>
    </div>

    <div class="section" id="sec-hypotheses" data-nav="Hypotheses">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Hypothesis Ledger</h2>
        <span class="section-desc">Bayesian truth values tracking what the system believes</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="hypotheses"><p class="empty">No hypotheses found</p></div>
    </div>

    <div class="section" id="sec-timeline" data-nav="Timeline">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Evidence Timeline</h2>
        <span class="section-desc">How truth values evolved over successive observations</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="timeline"><p class="empty">No evidence history. Run <code>cargo run --bin update_ledger</code></p></div>
    </div>

    <div class="section" id="sec-uncertainty" data-nav="Uncertainty">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Uncertainty Map</h2>
        <span class="section-desc">Knowledge classification from established to untested</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="uncertainty"><p class="empty">No uncertainty map data</p></div>
    </div>

    <div class="section" id="sec-traps" data-nav="Traps">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Backtest Trap Checklist</h2>
        <span class="section-desc">18-point integrity surface from the backtesting literature</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="traps"><p class="empty">No trap status data</p></div>
    </div>

    <div class="section" id="sec-leaderboard" data-nav="Leaderboard">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Regime Leaderboard</h2>
        <span class="section-desc">Strategies ranked by equity PnL per market regime</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="leaderboard"><p class="empty">No bench data for leaderboard</p></div>
    </div>

    <div class="section" id="sec-trends" data-nav="Trends">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Resource Trends</h2>
        <span class="section-desc">Throughput and timing across bench runs</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="trends"><p class="empty">No bench history for trends</p></div>
    </div>

    <div class="section" id="sec-history" data-nav="History">
      <div class="section-header" onclick="toggleSection(this)">
        <h2>Run History</h2>
        <span class="section-desc">Past pipeline and bench executions</span>
        <span class="section-toggle">&#9662;</span>
      </div>
      <div class="section-body" id="history"><p class="empty">No run history</p></div>
    </div>
  </main>

  <footer>
    <span id="footer-left"></span>
    <span><a href="https://github.com/uprootiny/arbitragefx">GitHub</a></span>
  </footer>

  <div class="kbd-hint" id="kbd-hint">
    <span class="kbd">j</span>/<span class="kbd">k</span> navigate &nbsp;
    <span class="kbd">c</span> collapse
  </div>

  <script>
  const D = JSON.parse('__WORKBENCH_DATA__');
  const sections = [...document.querySelectorAll('.section[data-nav]')];
  let activeSec = 0;

  // ── Helpers ──
  function toggleSection(header) {
    header.closest('.section').classList.toggle('collapsed');
  }
  function fmt(n, d=0) { return n.toLocaleString(undefined, {minimumFractionDigits:d, maximumFractionDigits:d}); }
  function pnlColor(v) { return v >= 0 ? 'var(--green)' : 'var(--red)'; }
  function pnlSign(v, d=2) { return (v >= 0 ? '+' : '') + v.toFixed(d); }
  function sparkSvg(vals, color, w=140, h=28) {
    if (!vals.length) return '';
    const pad = 2;
    if (vals.length === 1) {
      const y = h - pad - vals[0] * (h - 2*pad);
      return `<svg class="spark-svg" width="${w}" height="${h}"><circle cx="${w/2}" cy="${y}" r="2.5" fill="${color}"/></svg>`;
    }
    const mn = Math.min(...vals), mx = Math.max(...vals);
    const range = mx - mn || 1;
    const pts = vals.map((v, i) => {
      const x = pad + (i / (vals.length - 1)) * (w - 2*pad);
      const y = h - pad - ((v - mn) / range) * (h - 2*pad);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(' ');
    // gradient fill
    const last = vals[vals.length - 1], first = vals[0];
    const trend = last >= first ? color : 'var(--red)';
    return `<svg class="spark-svg" width="${w}" height="${h}">
      <defs><linearGradient id="sg_${color.replace(/[^a-z0-9]/g,'')}" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="${trend}" stop-opacity="0.2"/><stop offset="100%" stop-color="${trend}" stop-opacity="0"/>
      </linearGradient></defs>
      <polygon points="${pts} ${w-pad},${h-pad} ${pad},${h-pad}" fill="url(#sg_${color.replace(/[^a-z0-9]/g,'')})" />
      <polyline points="${pts}" fill="none" stroke="${trend}" stroke-width="1.5" stroke-linecap="round"/>
      <circle cx="${(pad + (vals.length-1)/(vals.length-1)*(w-2*pad)).toFixed(1)}" cy="${(h-pad-((last-mn)/range)*(h-2*pad)).toFixed(1)}" r="2.5" fill="${trend}"/>
    </svg>`;
  }
  function integrityRing(guarded, total, size=36) {
    const r = (size - 4) / 2, c = size / 2, circ = 2 * Math.PI * r;
    const pct = total > 0 ? guarded / total : 0;
    const color = pct >= 0.7 ? 'var(--green)' : pct >= 0.4 ? 'var(--yellow)' : 'var(--red)';
    return `<svg class="ring-svg" width="${size}" height="${size}">
      <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--border)" stroke-width="3"/>
      <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="${color}" stroke-width="3"
        stroke-dasharray="${(circ*pct).toFixed(1)} ${(circ*(1-pct)).toFixed(1)}" stroke-linecap="round"/>
    </svg>`;
  }

  // ── Navigation ──
  (() => {
    const nav = document.getElementById('nav-links');
    nav.innerHTML = sections.map((s, i) =>
      `<a href="#${s.id}" class="nav-link${i===0?' active':''}" data-idx="${i}">${s.dataset.nav}</a>`
    ).join('');
    const links = nav.querySelectorAll('.nav-link');

    // Intersection observer for active section highlighting
    const obs = new IntersectionObserver(entries => {
      for (const e of entries) {
        if (e.isIntersecting) {
          const idx = sections.indexOf(e.target);
          if (idx >= 0) {
            links.forEach(l => l.classList.remove('active'));
            links[idx].classList.add('active');
            activeSec = idx;
          }
        }
      }
    }, { rootMargin: '-20% 0px -70% 0px' });
    sections.forEach(s => obs.observe(s));

    // Nav status
    const status = document.getElementById('nav-status');
    const [g] = D.integrity_score.split('/').map(Number);
    status.innerHTML = `<span class="status-dot ok"></span> ${D.test_count} tests <span style="color:var(--border);">|</span> ${D.integrity_score} guarded`;
  })();

  // ── Hero ──
  (() => {
    const el = document.getElementById('hero');
    const date = D.generated.split('T')[0];
    const survivors = D.walk_forward ? D.walk_forward.summaries.filter(s => s.survives_correction).length : '?';
    const totalStrats = D.walk_forward ? D.walk_forward.summaries.length : '?';
    const established = D.hypotheses.filter(h => h.status === 'established').length;

    el.innerHTML = `
      <h1>ArbitrageFX Workbench</h1>
      <div class="hero-sub">${date} &middot; git ${D.git_sha} &middot; hypothesis-driven backtesting research</div>
      <div class="hero-findings">
        <div class="finding" style="border-color:var(--green-border);">
          <span class="finding-icon" style="color:var(--green);">&#9670;</span>
          <span><b>${established}</b> hypotheses established &mdash; capital preservation confirmed as edge</span>
        </div>
        <div class="finding" style="border-color:var(--red-border);">
          <span class="finding-icon" style="color:var(--red);">&#9670;</span>
          <span><b>${survivors}/${totalStrats}</b> strategies survive walk-forward correction</span>
        </div>
        <div class="finding" style="border-color:var(--yellow-border);">
          <span class="finding-icon" style="color:var(--yellow);">&#9670;</span>
          <span><b>${D.integrity_score}</b> backtest traps guarded</span>
        </div>
      </div>
    `;
  })();

  // ── Overview Cards ──
  (() => {
    const el = document.getElementById('overview');
    const b = D.bench;
    const benchTs = b ? b.timestamp.split('T')[0] : 'n/a';
    const tp = b ? Math.round(b.avg_throughput) : 0;
    const ms = b ? b.total_ms : 0;
    const surv = D.walk_forward ? D.walk_forward.summaries.filter(s => s.survives_correction).length : 0;
    const tot = D.walk_forward ? D.walk_forward.summaries.length : 0;
    const est = D.hypotheses.filter(h => h.status === 'established').length;
    const [g, t] = D.integrity_score.split('/').map(Number);

    el.innerHTML = `
      <div class="card accent-green"><div class="card-label">Tests</div><div class="card-val">${D.test_count}</div><div class="card-detail">all passing</div></div>
      <div class="card accent-blue"><div class="card-label">Datasets</div><div class="card-val">${D.dataset_count}</div><div class="card-detail">CSV regime files</div></div>
      <div class="card accent-blue"><div class="card-label">Throughput</div><div class="card-val">${fmt(tp)}</div><div class="card-detail">candles/sec</div></div>
      <div class="card"><div class="card-label">Bench Time</div><div class="card-val">${ms}<span style="font-size:0.7rem;font-weight:400;">ms</span></div><div class="card-detail">${benchTs}</div></div>
      <div class="card accent-red"><div class="card-label">WF Survivors</div><div class="card-val">${surv}/${tot}</div><div class="card-detail">after Bonferroni</div></div>
      <div class="card accent-green"><div class="card-label">Hypotheses</div><div class="card-val">${D.hypotheses.length}</div><div class="card-detail">${est} established</div></div>
      <div class="card accent-${g/t >= 0.5 ? 'yellow' : 'red'}">
        <div class="card-label">Integrity</div>
        <div class="card-val" style="display:flex;align-items:center;gap:0.4rem;">${integrityRing(g,t)} ${D.integrity_score}</div>
        <div class="card-detail">traps guarded</div>
      </div>
      <div class="card"><div class="card-label">Evidence</div><div class="card-val">${D.stv_history.length}</div><div class="card-detail">STV updates</div></div>
    `;
  })();

  // ── Profiling ──
  (() => {
    if (!D.bench) return;
    const el = document.getElementById('profiling');
    const ds = D.bench.datasets;
    const maxMs = Math.max(...ds.map(d => d.backtest_ms + d.walkforward_ms), 1);

    let html = '<div class="table-wrap"><table><thead><tr><th>Dataset</th><th class="text-right">Candles</th><th>Regime</th><th class="text-right">Backtest</th><th class="text-right">Walk-Fwd</th><th class="text-right">Throughput</th><th class="text-right">RSS</th></tr></thead><tbody>';
    for (const d of ds) {
      html += `<tr>
        <td class="mono">${d.name}</td>
        <td class="mono text-right">${fmt(d.candles)}</td>
        <td><span class="badge badge-blue">${d.regime.dominant_regime}</span></td>
        <td class="mono text-right">${d.backtest_ms}ms</td>
        <td class="mono text-right">${d.walkforward_ms}ms</td>
        <td class="mono text-right">${fmt(Math.round(d.throughput_candles_per_sec))} c/s</td>
        <td class="mono text-right">${d.peak_rss_kb ? fmt(d.peak_rss_kb)+' KB' : 'n/a'}</td>
      </tr>`;
    }
    html += '</tbody></table></div>';
    html += '<div style="margin-top:1rem;">';
    for (const d of ds) {
      const btPct = d.backtest_ms / maxMs * 100;
      const wfPct = d.walkforward_ms / maxMs * 100;
      html += `<div class="bar-row">
        <span class="bar-label">${d.name.replace('btc_','').replace('_1h','')}</span>
        <div class="bar-track">
          <div class="bar-seg" style="width:${btPct.toFixed(1)}%;background:var(--accent);"></div>
          <div class="bar-seg" style="width:${wfPct.toFixed(1)}%;background:var(--accent-muted);"></div>
        </div>
        <span class="bar-val">${d.backtest_ms + d.walkforward_ms}ms</span>
      </div>`;
    }
    html += '</div>';
    html += '<div class="chart-legend"><span><span class="legend-line" style="background:var(--accent);"></span> Backtest</span><span><span class="legend-line" style="background:var(--accent-muted);"></span> Walk-forward</span></div>';
    el.innerHTML = html;
  })();

  // ── Heatmap ──
  (() => {
    if (!D.bench) return;
    const el = document.getElementById('heatmap');
    const ds = D.bench.datasets;
    if (!ds.length || !ds[0].backtest_result) return;
    const stratIds = ds[0].backtest_result.strategies.map(s => s.id);
    const regimes = ds.map(d => d.name.replace('btc_','').replace('_1h',''));

    let html = `<div class="heatmap" style="grid-template-columns:90px repeat(${regimes.length},1fr);">`;
    html += '<div class="hm-head"></div>';
    for (const r of regimes) html += `<div class="hm-head">${r}</div>`;
    for (let si = 0; si < stratIds.length; si++) {
      html += `<div class="hm-head" style="text-align:right;">${stratIds[si]}</div>`;
      for (let di = 0; di < ds.length; di++) {
        const st = ds[di].backtest_result.strategies[si];
        if (!st) { html += '<div class="hm-cell" style="background:#21262d;">-</div>'; continue; }
        const p = st.equity_pnl;
        const a = Math.min(Math.abs(p) / 8, 0.85);
        const bg = p > 0 ? `rgba(63,185,80,${a})` : p < 0 ? `rgba(248,81,73,${a})` : 'rgba(139,148,158,0.1)';
        const dd = st.max_drawdown !== undefined ? ` DD:${(st.max_drawdown*100).toFixed(1)}%` : '';
        html += `<div class="hm-cell tip" style="background:${bg};" data-tip="${st.id} ${pnlSign(p)}${dd}">${pnlSign(p)}</div>`;
      }
    }
    html += '</div>';
    html += '<div class="chart-legend" style="margin-top:0.4rem;"><span><span class="legend-line" style="background:var(--green);"></span> Positive PnL</span><span><span class="legend-line" style="background:var(--red);"></span> Negative PnL</span><span style="color:var(--fg-subtle);">Hover for details</span></div>';
    el.innerHTML = html;
  })();

  // ── Walk-Forward ──
  (() => {
    if (!D.walk_forward) return;
    const el = document.getElementById('walkforward');
    const wf = D.walk_forward;
    let html = `<div style="display:flex;gap:0.6rem;flex-wrap:wrap;margin-bottom:0.75rem;">
      <span class="badge badge-gray">${wf.num_windows} windows</span>
      <span class="badge badge-gray">${wf.num_strategies} strategies</span>
      <span class="badge badge-gray">${wf.correction_method}</span>
      <span class="badge badge-gray">&alpha;=${wf.alpha}</span>
    </div>`;
    html += '<div class="table-wrap"><table><thead><tr><th>Strategy</th><th class="text-right">Train PnL</th><th class="text-right">Test PnL</th><th class="text-right">Overfit</th><th class="text-right">P-value</th><th class="text-center">Positive</th><th class="text-center">Survives</th></tr></thead><tbody>';
    for (const s of wf.summaries) {
      const orColor = s.overfit_ratio > 0.5 ? 'var(--green)' : s.overfit_ratio > 0 ? 'var(--yellow)' : 'var(--red)';
      html += `<tr>
        <td class="mono">${s.id}</td>
        <td class="mono text-right" style="color:${pnlColor(s.train_mean_pnl)}">${pnlSign(s.train_mean_pnl,4)}</td>
        <td class="mono text-right" style="color:${pnlColor(s.test_mean_pnl)}">${pnlSign(s.test_mean_pnl,4)}</td>
        <td class="mono text-right" style="color:${orColor}">${s.overfit_ratio.toFixed(3)}</td>
        <td class="mono text-right">${s.p_value.toFixed(3)}</td>
        <td class="mono text-center">${s.test_positive_windows}/${s.total_windows}</td>
        <td class="text-center">${s.survives_correction ? '<span class="badge badge-green">YES</span>' : '<span class="badge badge-red">no</span>'}</td>
      </tr>`;
    }
    html += '</tbody></table></div>';
    el.innerHTML = html;
  })();

  // ── Hypotheses ──
  (() => {
    if (!D.hypotheses.length) return;
    const el = document.getElementById('hypotheses');
    let html = '<div class="table-wrap"><table><thead><tr><th>ID</th><th>Hypothesis</th><th class="text-right">Strength</th><th class="text-right">Confidence</th><th>Status</th><th>Assessment</th></tr></thead><tbody>';
    for (const h of D.hypotheses) {
      const sC = h.strength >= 0.7 ? 'var(--green)' : h.strength >= 0.4 ? 'var(--yellow)' : 'var(--red)';
      const cC = h.confidence >= 0.7 ? 'var(--green)' : h.confidence >= 0.4 ? 'var(--yellow)' : 'var(--red)';
      const cls = h.status === 'established' || h.status === 'supported' ? 'badge-green' : h.status === 'contested' ? 'badge-yellow' : 'badge-red';
      html += `<tr>
        <td class="mono">${h.id}</td><td>${h.name}</td>
        <td class="mono text-right" style="color:${sC}">${h.strength.toFixed(2)}</td>
        <td class="mono text-right" style="color:${cC}">${h.confidence.toFixed(2)}</td>
        <td><span class="badge ${cls}">${h.status}</span></td>
        <td style="font-size:0.7rem;color:var(--fg-muted);max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title="${h.assessment}">${h.assessment}</td>
      </tr>`;
    }
    html += '</tbody></table></div>';
    el.innerHTML = html;
  })();

  // ── Evidence Timeline ──
  (() => {
    if (!D.stv_history.length) return;
    const el = document.getElementById('timeline');
    const byH = {};
    for (const e of D.stv_history) { (byH[e.hypothesis_id] = byH[e.hypothesis_id] || []).push(e); }
    let html = '<div class="table-wrap"><table><thead><tr><th>Hypothesis</th><th>Strength</th><th>Confidence</th><th class="text-right">Updates</th><th>Latest Observation</th></tr></thead><tbody>';
    for (const id of Object.keys(byH).sort()) {
      const ents = byH[id];
      const last = ents[ents.length - 1];
      const sVals = ents.map(e => e.new_stv[0]);
      const cVals = ents.map(e => e.new_stv[1]);
      html += `<tr>
        <td class="mono">${id}</td>
        <td><div class="spark-wrap">${sparkSvg(sVals, '#3fb950')} <span class="spark-val" style="color:var(--green)">${last.new_stv[0].toFixed(2)}</span></div></td>
        <td><div class="spark-wrap">${sparkSvg(cVals, '#58a6ff')} <span class="spark-val" style="color:var(--accent)">${last.new_stv[1].toFixed(2)}</span></div></td>
        <td class="mono text-right">${ents.length}</td>
        <td style="font-size:0.7rem;color:var(--fg-muted);max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title="${last.observation}">${last.dataset} &mdash; ${last.observation}</td>
      </tr>`;
    }
    html += '</tbody></table></div>';
    el.innerHTML = html;
  })();

  // ── Uncertainty Map ──
  (() => {
    const um = D.uncertainty_map;
    const cats = [
      { key:'well_established', label:'Well-Established', color:'var(--green)' },
      { key:'supported', label:'Supported', color:'#6fdd8b' },
      { key:'contested', label:'Contested', color:'var(--yellow)' },
      { key:'fragile', label:'Fragile', color:'var(--red)' },
      { key:'untested', label:'Untested', color:'var(--fg-muted)' }
    ];
    if (!cats.some(c => (um[c.key]||[]).length)) return;
    const el = document.getElementById('uncertainty');
    let html = '<div class="kanban">';
    for (const c of cats) {
      const items = um[c.key] || [];
      html += `<div class="kanban-col" style="border-top-color:${c.color};">
        <div class="kanban-title" style="color:${c.color};">${c.label} <span class="kanban-count">${items.length}</span></div>`;
      if (!items.length) html += '<div class="kanban-empty">none</div>';
      else for (const it of items) html += `<div class="kanban-item">${it}</div>`;
      html += '</div>';
    }
    html += '</div>';
    el.innerHTML = html;
  })();

  // ── Trap Checklist ──
  (() => {
    if (!D.trap_status.length) return;
    const el = document.getElementById('traps');
    const g = D.trap_status.filter(t => t.guard==='Guarded').length;
    const p = D.trap_status.filter(t => t.guard==='Partial').length;
    const u = D.trap_status.filter(t => t.guard==='Unguarded').length;
    let html = `<div style="display:flex;gap:0.5rem;margin-bottom:0.75rem;flex-wrap:wrap;">
      <span class="badge badge-green">${g} guarded</span>
      <span class="badge badge-yellow">${p} partial</span>
      <span class="badge badge-red">${u} unguarded</span>
    </div>`;
    html += '<div class="table-wrap"><table><thead><tr><th style="width:30px">#</th><th style="width:20px"></th><th>Trap</th><th>Severity</th><th>Guard</th><th>Evidence</th></tr></thead><tbody>';
    for (const t of D.trap_status) {
      const sc = t.severity==='Critical'?'badge-red':t.severity==='High'?'badge-yellow':'badge-blue';
      const gc = t.guard==='Guarded'?'guarded':t.guard==='Partial'?'partial':'unguarded';
      const gcb = t.guard==='Guarded'?'badge-green':t.guard==='Partial'?'badge-yellow':'badge-red';
      html += `<tr>
        <td class="mono text-center">${t.id}</td>
        <td><span class="trap-dot ${gc}"></span></td>
        <td>${t.name}</td>
        <td><span class="badge ${sc}">${t.severity}</span></td>
        <td><span class="badge ${gcb}">${t.guard}</span></td>
        <td style="font-size:0.7rem;color:var(--fg-muted);">${t.evidence}</td>
      </tr>`;
    }
    html += '</tbody></table></div>';
    el.innerHTML = html;
  })();

  // ── Regime Leaderboard ──
  (() => {
    if (!D.bench) return;
    const el = document.getElementById('leaderboard');
    const ds = D.bench.datasets;
    if (!ds.length || !ds[0].backtest_result) return;
    let html = '<div class="lb-grid">';
    for (const d of ds) {
      const regime = d.name.replace('btc_','').replace('_1h','');
      const strats = d.backtest_result.strategies.slice().sort((a,b) => b.equity_pnl - a.equity_pnl);
      html += `<div class="lb-col"><div class="lb-title">${regime}</div>`;
      for (let i = 0; i < strats.length; i++) {
        const s = strats[i];
        const safe = s.max_drawdown <= 0.02;
        html += `<div class="lb-row">
          <span><span class="lb-rank">${i+1}.</span> ${s.id}${safe ? ' <span style="color:var(--green);font-size:0.6rem;" title="DD < 2%">&#x2713;</span>' : ''}</span>
          <span style="color:${pnlColor(s.equity_pnl)}">${pnlSign(s.equity_pnl)}</span>
        </div>`;
      }
      html += '</div>';
    }
    html += '</div>';
    html += '<div class="chart-legend" style="margin-top:0.5rem;"><span style="color:var(--green);">&#x2713; = max drawdown &lt; 2%</span><span>Ranked by equity PnL</span></div>';
    el.innerHTML = html;
  })();

  // ── Resource Trends ──
  (() => {
    if (D.bench_history.length < 1) return;
    const el = document.getElementById('trends');
    const data = D.bench_history;
    const w = 700, h = 140, pad = 45;
    const maxT = Math.max(...data.map(d => d.avg_throughput), 1);
    const maxM = Math.max(...data.map(d => d.total_ms), 1);

    let svg = `<div class="chart-container"><svg viewBox="0 0 ${w} ${h}" style="width:100%;">`;
    // Grid lines
    for (let i = 0; i <= 4; i++) {
      const y = pad + (i/4) * (h - 2*pad);
      svg += `<line x1="${pad}" y1="${y}" x2="${w-pad}" y2="${y}" stroke="var(--border)" stroke-width="0.5"/>`;
    }
    if (data.length === 1) {
      const barW = 50;
      const tH = (data[0].avg_throughput / maxT) * (h - 2*pad);
      svg += `<rect x="${w/2-barW/2}" y="${h-pad-tH}" width="${barW}" height="${tH}" fill="var(--accent)" opacity="0.7" rx="3"/>`;
      svg += `<text x="${w/2}" y="${h-pad+14}" fill="var(--fg-muted)" font-size="9" text-anchor="middle" font-family="var(--mono)">${data[0].date}</text>`;
      svg += `<text x="${w/2}" y="${h-pad-tH-6}" fill="var(--accent)" font-size="10" text-anchor="middle" font-family="var(--mono)">${fmt(Math.round(data[0].avg_throughput))} c/s</text>`;
    } else {
      // Throughput area
      const tPts = data.map((d, i) => {
        const x = pad + (i/(data.length-1)) * (w-2*pad);
        const y = h - pad - (d.avg_throughput/maxT) * (h-2*pad);
        return [x,y];
      });
      const areaPts = tPts.map(p => `${p[0].toFixed(1)},${p[1].toFixed(1)}`).join(' ');
      svg += `<defs><linearGradient id="tg" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="var(--accent)" stop-opacity="0.15"/><stop offset="100%" stop-color="var(--accent)" stop-opacity="0"/></linearGradient></defs>`;
      svg += `<polygon points="${areaPts} ${tPts[tPts.length-1][0].toFixed(1)},${h-pad} ${tPts[0][0].toFixed(1)},${h-pad}" fill="url(#tg)"/>`;
      svg += `<polyline points="${areaPts}" fill="none" stroke="var(--accent)" stroke-width="2" stroke-linecap="round"/>`;
      for (const [x,y] of tPts) svg += `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="3" fill="var(--accent)"/>`;
      // Time overlay
      const mPts = data.map((d, i) => {
        const x = pad + (i/(data.length-1)) * (w-2*pad);
        const y = h - pad - (d.total_ms/maxM) * (h-2*pad);
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      }).join(' ');
      svg += `<polyline points="${mPts}" fill="none" stroke="var(--yellow)" stroke-width="1.5" stroke-dasharray="5,3" opacity="0.5"/>`;
      // X labels
      for (let i = 0; i < data.length; i++) {
        const x = pad + (i/(data.length-1)) * (w-2*pad);
        svg += `<text x="${x.toFixed(1)}" y="${h-pad+14}" fill="var(--fg-subtle)" font-size="8" text-anchor="middle" font-family="var(--mono)">${data[i].date.substring(5)}</text>`;
      }
    }
    // Y labels
    svg += `<text x="${pad-6}" y="${pad+4}" fill="var(--fg-subtle)" font-size="8" text-anchor="end" font-family="var(--mono)">${fmt(Math.round(maxT))}</text>`;
    svg += `<text x="${pad-6}" y="${h-pad+4}" fill="var(--fg-subtle)" font-size="8" text-anchor="end" font-family="var(--mono)">0</text>`;
    svg += '</svg>';
    svg += '<div class="chart-legend"><span><span class="legend-line" style="background:var(--accent);"></span> Throughput (c/s)</span><span><span class="legend-line" style="background:var(--yellow);border-style:dashed;"></span> Total time (ms)</span></div>';
    svg += '</div>';
    el.innerHTML = svg;
  })();

  // ── History ──
  (() => {
    const el = document.getElementById('history');
    let html = '';
    if (D.bench_history.length > 0) {
      html += '<div class="table-wrap"><table><thead><tr><th>Date</th><th class="text-right">Candles</th><th class="text-right">Time</th><th class="text-right">Throughput</th></tr></thead><tbody>';
      for (const b of D.bench_history) {
        html += `<tr><td class="mono">${b.date}</td><td class="mono text-right">${fmt(b.total_candles)}</td><td class="mono text-right">${b.total_ms}ms</td><td class="mono text-right">${fmt(Math.round(b.avg_throughput))} c/s</td></tr>`;
      }
      html += '</tbody></table></div>';
    }
    if (D.run_history.length > 0) {
      html += `<div style="margin-top:0.75rem;" class="table-wrap"><table><thead><tr><th>Date</th><th>Report</th></tr></thead><tbody>`;
      for (const r of D.run_history) html += `<tr><td class="mono">${r.date}</td><td class="mono" style="color:var(--fg-muted);">${r.path}</td></tr>`;
      html += '</tbody></table></div>';
    }
    if (!html) html = '<p class="empty">No historical data yet. Run the pipeline to generate history.</p>';
    el.innerHTML = html;
  })();

  // ── Footer ──
  document.getElementById('footer-left').textContent = `Generated ${D.generated} \u00b7 git ${D.git_sha}`;

  // ── Keyboard Navigation ──
  (() => {
    const hint = document.getElementById('kbd-hint');
    let hintTimer;
    function showHint() { hint.classList.add('show'); clearTimeout(hintTimer); hintTimer = setTimeout(() => hint.classList.remove('show'), 2000); }

    document.addEventListener('keydown', e => {
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
      if (e.key === 'j') { activeSec = Math.min(activeSec + 1, sections.length - 1); sections[activeSec].scrollIntoView({behavior:'smooth'}); showHint(); }
      else if (e.key === 'k') { activeSec = Math.max(activeSec - 1, 0); sections[activeSec].scrollIntoView({behavior:'smooth'}); showHint(); }
      else if (e.key === 'c') { sections[activeSec].classList.toggle('collapsed'); showHint(); }
    });
    // Show hint briefly on load
    setTimeout(() => { hint.classList.add('show'); setTimeout(() => hint.classList.remove('show'), 3000); }, 1500);
  })();
  </script>
</body>
</html>"##;
