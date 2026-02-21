//! Epistemic Dashboard Server
//!
//! Serves the epistemic state of the system as JSON for the ClojureScript dashboard.
//! Run with: cargo run --bin epistemic_server

use arbitragefx::epistemic::EpistemicState;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

/// Count #[test] attributes in source files (fast, no cargo invocation).
fn count_tests_fast() -> usize {
    let mut count = 0;
    for dir in &["tests", "src"] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        count += content.matches("#[test]").count();
                    }
                }
            }
        }
    }
    count
}

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(51723);
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind");

    // Pre-compute expensive values at startup
    let cached_test_count = count_tests_fast();
    println!("  Cached test count: {}", cached_test_count);

    // Pre-compute trap status
    let traps = arbitragefx::backtest_traps::trap_status();
    let cached_traps_json = serde_json::json!(traps
        .iter()
        .map(|t| serde_json::json!({
            "id": t.id,
            "name": t.name,
            "severity": format!("{:?}", t.severity),
            "guard": format!("{:?}", t.guard),
            "evidence": t.evidence,
        }))
        .collect::<Vec<_>>())
    .to_string();
    println!("  Cached trap status: {} traps", traps.len());

    // Pre-load STV history
    let cached_timeline_json = std::fs::read_to_string("out/ledger_history/updates.jsonl")
        .map(|content| {
            let entries: Vec<serde_json::Value> = content
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
        })
        .unwrap_or_else(|_| "[]".into());
    println!("  Cached timeline entries");

    // Pre-load workbench HTML for serving
    let workbench_html = std::fs::read_to_string("docs/workbench.html")
        .or_else(|_| std::fs::read_to_string("out/workbench/index.html"))
        .unwrap_or_else(|_| {
            "<html><body><h1>Workbench not generated</h1><p>Run: cargo run --release --bin workbench</p></body></html>".into()
        });

    println!();
    println!("Epistemic Server running at http://localhost:{}", port);
    println!();
    println!("Endpoints:");
    println!("  GET /               - Workbench dashboard");
    println!("  GET /api/state      - Full epistemic state");
    println!("  GET /api/health     - Enriched health check");
    println!("  GET /api/summary    - Compact status");
    println!("  GET /api/traps      - 18-point trap checklist");
    println!("  GET /api/timeline   - STV evidence history");
    println!();

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let buf_reader = BufReader::new(&stream);
        let request_line = buf_reader.lines().next();

        let request = match request_line {
            Some(Ok(line)) => line,
            _ => continue,
        };

        let (status, content_type, body) = if request.starts_with("GET / ")
            || request.starts_with("GET / HTTP")
            || request == "GET /"
        {
            ("200 OK", "text/html; charset=utf-8", workbench_html.clone())
        } else if request.starts_with("GET /api/state") {
            let state = EpistemicState::from_system();
            ("200 OK", "application/json", state.to_json())
        } else if request.starts_with("GET /api/health") {
            let state = EpistemicState::from_system();
            let (guarded, total) = arbitragefx::backtest_traps::integrity_score();

            let test_count = cached_test_count;

            // Last pipeline run date
            let last_pipeline = std::fs::read_dir("out/reports")
                .ok()
                .and_then(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "txt")
                                .unwrap_or(false)
                        })
                        .filter_map(|e| {
                            e.path()
                                .file_stem()
                                .and_then(|s| s.to_str().map(|s| s.to_string()))
                        })
                        .max()
                })
                .unwrap_or_else(|| "never".into());

            // Dataset row counts
            let datasets: serde_json::Value = std::fs::read_dir("data")
                .map(|rd| {
                    let mut map = serde_json::Map::new();
                    for entry in rd.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.extension().map(|e| e == "csv").unwrap_or(false) {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let rows = std::fs::read_to_string(&path)
                                .map(|c| c.lines().count().saturating_sub(1)) // exclude header
                                .unwrap_or(0);
                            map.insert(name, serde_json::json!({"rows": rows}));
                        }
                    }
                    serde_json::Value::Object(map)
                })
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            let health = serde_json::json!({
                "status": "ok",
                "test_count": test_count,
                "invariants_hold": state.invariants_hold(),
                "last_pipeline": last_pipeline,
                "integrity_score": format!("{}/{}", guarded, total),
                "datasets": datasets,
                "hypotheses": state.hypotheses.len(),
            });
            ("200 OK", "application/json", health.to_string())
        } else if request.starts_with("GET /api/traps") {
            ("200 OK", "application/json", cached_traps_json.clone())
        } else if request.starts_with("GET /api/timeline") {
            ("200 OK", "application/json", cached_timeline_json.clone())
        } else if request.starts_with("GET /api/summary") {
            let state = EpistemicState::from_system();
            let counts = state.count_by_level();
            let summary = serde_json::json!({
                "hypotheses": state.hypotheses.len(),
                "signals": state.signals.len(),
                "filters": state.filters.len(),
                "invariants_hold": state.invariants_hold(),
                "by_level": counts.iter()
                    .map(|(k, v)| (format!("{:?}", k), v))
                    .collect::<std::collections::HashMap<_, _>>()
            });
            ("200 OK", "application/json", summary.to_string())
        } else {
            ("404 NOT FOUND", "text/plain", "Not Found".to_string())
        };

        let response = format!(
            "HTTP/1.1 {}\r\n\
             Content-Type: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Content-Length: {}\r\n\r\n{}",
            status,
            content_type,
            body.len(),
            body
        );

        let _ = stream.write_all(response.as_bytes());
    }
}
