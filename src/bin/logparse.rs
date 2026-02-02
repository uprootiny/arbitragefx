//! Log parsing and analysis tool for arbitragefx logs.
//!
//! Usage:
//!   logparse <command> [options]
//!
//! Commands:
//!   summary <file.jsonl>           - Summarize a log file
//!   filter <file.jsonl> [options]  - Filter logs by domain/level
//!   replay <file.jsonl>            - Validate replay determinism
//!   audit <file.jsonl>             - Check audit trail integrity
//!   slice <file.jsonl> <start> <end> - Extract time slice
//!
//! Options:
//!   --domain=<domain>    Filter by domain (market,strategy,risk,exec,fill,drift,system,audit,agent)
//!   --level=<level>      Minimum level (trace,debug,info,warn,error,fatal)
//!   --json               Output as JSON (default: human-readable)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct LogEntry {
    ts: String,
    run_id: String,
    seq: u64,
    lvl: String,
    component: String,
    event: String,
    msg: Option<String>,
    #[serde(default)]
    data: Value,
    // Optional fields that may be at top level
    strategy_id: Option<String>,
    symbol: Option<String>,
    intent_id: Option<String>,
}

#[derive(Debug, Default)]
struct LogStats {
    total_entries: u64,
    by_level: HashMap<String, u64>,
    by_domain: HashMap<String, u64>,
    by_event: HashMap<String, u64>,
    first_ts: Option<String>,
    last_ts: Option<String>,
    first_seq: Option<u64>,
    last_seq: Option<u64>,
    // Trading-specific
    trades: u64,
    fills: u64,
    decisions: u64,
    risk_blocks: u64,
    drift_events: u64,
    errors: u64,
}

#[derive(Debug, Clone)]
struct FilterConfig {
    domains: Option<Vec<String>>,
    min_level: Option<String>,
    events: Option<Vec<String>>,
    strategy_id: Option<String>,
}

fn level_rank(lvl: &str) -> u8 {
    match lvl.to_lowercase().as_str() {
        "trace" => 0,
        "debug" => 1,
        "info" => 2,
        "warn" => 3,
        "error" => 4,
        "fatal" => 5,
        _ => 2,
    }
}

fn parse_log_file(path: &PathBuf) -> impl Iterator<Item = (String, Option<LogEntry>)> {
    let file = File::open(path).expect("Failed to open log file");
    BufReader::new(file).lines().map(|line| {
        let line = line.unwrap_or_default();
        let parsed = serde_json::from_str::<LogEntry>(&line).ok();
        (line, parsed)
    })
}

fn cmd_summary(path: &PathBuf) {
    let mut stats = LogStats::default();

    for (line, entry) in parse_log_file(path) {
        if let Some(e) = entry {
            stats.total_entries += 1;
            *stats.by_level.entry(e.lvl.clone()).or_insert(0) += 1;
            *stats.by_domain.entry(e.component.clone()).or_insert(0) += 1;
            *stats.by_event.entry(e.event.clone()).or_insert(0) += 1;

            if stats.first_ts.is_none() {
                stats.first_ts = Some(e.ts.clone());
                stats.first_seq = Some(e.seq);
            }
            stats.last_ts = Some(e.ts.clone());
            stats.last_seq = Some(e.seq);

            // Count special events
            match e.event.as_str() {
                "trade" | "order_submit" => stats.trades += 1,
                "fill" => stats.fills += 1,
                "decision" => stats.decisions += 1,
                "guard" if e.data.get("result").and_then(|v| v.as_str()) == Some("blocked") => {
                    stats.risk_blocks += 1
                }
                "drift_detected" => stats.drift_events += 1,
                _ => {}
            }
            if e.lvl == "ERROR" || e.lvl == "FATAL" {
                stats.errors += 1;
            }
        } else if !line.is_empty() {
            eprintln!("Failed to parse: {}", &line[..line.len().min(80)]);
        }
    }

    println!("=== Log Summary ===\n");
    println!("Total entries: {}", stats.total_entries);
    println!(
        "Time range: {} → {}",
        stats.first_ts.as_deref().unwrap_or("?"),
        stats.last_ts.as_deref().unwrap_or("?")
    );
    println!(
        "Sequence range: {} → {}",
        stats.first_seq.unwrap_or(0),
        stats.last_seq.unwrap_or(0)
    );

    println!("\n--- By Level ---");
    let mut levels: Vec<_> = stats.by_level.iter().collect();
    levels.sort_by_key(|(k, _)| level_rank(k));
    for (lvl, count) in levels {
        println!("  {:<8} {:>8}", lvl, count);
    }

    println!("\n--- By Domain ---");
    let mut domains: Vec<_> = stats.by_domain.iter().collect();
    domains.sort_by(|a, b| b.1.cmp(a.1));
    for (domain, count) in domains {
        println!("  {:<12} {:>8}", domain, count);
    }

    println!("\n--- Top Events ---");
    let mut events: Vec<_> = stats.by_event.iter().collect();
    events.sort_by(|a, b| b.1.cmp(a.1));
    for (event, count) in events.iter().take(15) {
        println!("  {:<24} {:>8}", event, count);
    }

    println!("\n--- Trading Activity ---");
    println!("  Decisions made:    {:>8}", stats.decisions);
    println!("  Orders submitted:  {:>8}", stats.trades);
    println!("  Fills received:    {:>8}", stats.fills);
    println!("  Risk blocks:       {:>8}", stats.risk_blocks);
    println!("  Drift events:      {:>8}", stats.drift_events);
    println!("  Errors:            {:>8}", stats.errors);
}

fn cmd_filter(path: &PathBuf, config: &FilterConfig, as_json: bool) {
    for (line, entry) in parse_log_file(path) {
        let Some(e) = entry else { continue };

        // Filter by level
        if let Some(ref min) = config.min_level {
            if level_rank(&e.lvl) < level_rank(min) {
                continue;
            }
        }

        // Filter by domain
        if let Some(ref domains) = config.domains {
            if !domains.iter().any(|d| d == &e.component) {
                continue;
            }
        }

        // Filter by event
        if let Some(ref events) = config.events {
            if !events.iter().any(|ev| ev == &e.event) {
                continue;
            }
        }

        // Filter by strategy
        if let Some(ref sid) = config.strategy_id {
            let matches = e.strategy_id.as_ref() == Some(sid)
                || e.data.get("strategy_id").and_then(|v| v.as_str()) == Some(sid);
            if !matches {
                continue;
            }
        }

        if as_json {
            println!("{}", line);
        } else {
            let msg = e.msg.as_deref().unwrap_or("");
            println!(
                "[{}] {} {} {} {}",
                &e.ts[11..23], // HH:MM:SS.mmm
                e.lvl,
                e.component,
                e.event,
                msg
            );
        }
    }
}

fn cmd_replay(path: &PathBuf) {
    println!("=== Replay Validation ===\n");

    let mut last_seq: Option<u64> = None;
    let mut seq_gaps = 0u64;
    let mut state_hashes: HashMap<String, String> = HashMap::new();
    let mut hash_changes = 0u64;
    let mut checkpoints = 0u64;

    for (_, entry) in parse_log_file(path) {
        let Some(e) = entry else { continue };

        // Check sequence continuity
        if let Some(prev) = last_seq {
            if e.seq != prev + 1 {
                seq_gaps += 1;
            }
        }
        last_seq = Some(e.seq);

        // Track state hashes
        if let Some(hash) = e.data.get("state_hash").and_then(|v| v.as_str()) {
            if e.event == "checkpoint" {
                checkpoints += 1;
            }
            let sid = e.strategy_id.clone().or_else(|| {
                e.data.get("strategy_id").and_then(|v| v.as_str()).map(|s| s.to_string())
            });
            if let Some(sid) = sid {
                if let Some(prev_hash) = state_hashes.get(&sid) {
                    if prev_hash != hash {
                        hash_changes += 1;
                    }
                }
                state_hashes.insert(sid, hash.to_string());
            }
        }
    }

    println!("Sequence analysis:");
    println!("  Last seq: {}", last_seq.unwrap_or(0));
    println!("  Gaps detected: {}", seq_gaps);

    println!("\nState tracking:");
    println!("  Checkpoints: {}", checkpoints);
    println!("  Strategies tracked: {}", state_hashes.len());
    println!("  State changes: {}", hash_changes);

    if seq_gaps == 0 {
        println!("\n✓ No sequence gaps - replay should be deterministic");
    } else {
        println!("\n⚠ {} sequence gaps - may affect replay", seq_gaps);
    }
}

fn cmd_audit(path: &PathBuf) {
    println!("=== Audit Trail Check ===\n");

    let mut entries: Vec<LogEntry> = Vec::new();
    let mut audit_count = 0u64;
    let mut decision_count = 0u64;
    let mut fill_count = 0u64;

    for (_, entry) in parse_log_file(path) {
        if let Some(e) = entry {
            match e.event.as_str() {
                "checkpoint" => audit_count += 1,
                "decision" => decision_count += 1,
                "fill" => fill_count += 1,
                _ => {}
            }
            entries.push(e);
        }
    }

    println!("Audit coverage:");
    println!("  Checkpoints:   {:>8}", audit_count);
    println!("  Decisions:     {:>8}", decision_count);
    println!("  Fills:         {:>8}", fill_count);

    // Check for required audit fields
    let mut missing_hash = 0u64;
    for e in &entries {
        if matches!(e.event.as_str(), "decision" | "order_submit" | "fill") {
            if e.data.get("state_hash").is_none() {
                missing_hash += 1;
            }
        }
    }

    if missing_hash == 0 {
        println!("\n✓ All auditable events have state hashes");
    } else {
        println!("\n⚠ {} events missing state_hash", missing_hash);
    }

    // Verify checkpoint-to-checkpoint integrity
    let checkpoints: Vec<_> = entries
        .iter()
        .filter(|e| e.event == "checkpoint")
        .collect();

    if checkpoints.len() >= 2 {
        println!("\nCheckpoint chain:");
        for (i, cp) in checkpoints.iter().take(5).enumerate() {
            let hash = cp.data.get("state_hash").and_then(|v| v.as_str()).unwrap_or("?");
            let pnl = cp.data.get("pnl").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("  #{}: hash={} pnl={:.2}", i + 1, &hash[..16.min(hash.len())], pnl);
        }
        if checkpoints.len() > 5 {
            println!("  ... and {} more", checkpoints.len() - 5);
        }
    }
}

fn cmd_slice(path: &PathBuf, start: &str, end: &str) {
    for (line, entry) in parse_log_file(path) {
        let Some(e) = entry else { continue };
        if e.ts >= start.to_string() && e.ts <= end.to_string() {
            println!("{}", line);
        }
    }
}

fn cmd_agent(path: &PathBuf) {
    println!("=== AI Agent Summary ===\n");

    let mut decisions = Vec::new();
    let mut contexts = Vec::new();
    let mut reasonings = Vec::new();

    for (_, entry) in parse_log_file(path) {
        let Some(e) = entry else { continue };
        match e.event.as_str() {
            "decision" => decisions.push(e),
            "market_context" => contexts.push(e),
            "reasoning" => reasonings.push(e),
            _ => {}
        }
    }

    println!("Decision log entries: {}", decisions.len());
    println!("Market context entries: {}", contexts.len());
    println!("Reasoning chains: {}", reasonings.len());

    if !decisions.is_empty() {
        println!("\n--- Recent Decisions ---");
        for d in decisions.iter().rev().take(5) {
            let intent = d.data.get("intent").and_then(|v| v.as_str()).unwrap_or("?");
            let conf = d.data.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let reason = d.data.get("reason").and_then(|v| v.as_str()).unwrap_or("?");
            println!("  {} (conf={:.2}): {}", intent, conf, reason);
        }
    }

    if !reasonings.is_empty() {
        println!("\n--- Sample Reasoning Chain ---");
        if let Some(r) = reasonings.last() {
            if let Some(steps) = r.data.get("steps").and_then(|v| v.as_array()) {
                for (i, step) in steps.iter().enumerate() {
                    if let Some(s) = step.as_str() {
                        println!("  {}. {}", i + 1, s);
                    }
                }
            }
        }
    }
}

fn print_usage() {
    eprintln!("Usage: logparse <command> <file.jsonl> [options]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  summary <file>              Summarize log file statistics");
    eprintln!("  filter <file> [options]     Filter and display log entries");
    eprintln!("  replay <file>               Validate replay determinism");
    eprintln!("  audit <file>                Check audit trail integrity");
    eprintln!("  slice <file> <start> <end>  Extract entries in time range");
    eprintln!("  agent <file>                Summarize AI agent guidance logs");
    eprintln!();
    eprintln!("Filter options:");
    eprintln!("  --domain=<d1,d2,...>   Filter by domain(s)");
    eprintln!("  --level=<level>        Minimum log level");
    eprintln!("  --event=<e1,e2,...>    Filter by event type(s)");
    eprintln!("  --strategy=<id>        Filter by strategy ID");
    eprintln!("  --json                 Output raw JSON lines");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        print_usage();
        std::process::exit(1);
    }

    let cmd = &args[1];
    let path = PathBuf::from(&args[2]);

    if !path.exists() {
        eprintln!("Error: File not found: {}", path.display());
        std::process::exit(1);
    }

    match cmd.as_str() {
        "summary" => cmd_summary(&path),
        "filter" => {
            let mut config = FilterConfig {
                domains: None,
                min_level: None,
                events: None,
                strategy_id: None,
            };
            let mut as_json = false;

            for arg in &args[3..] {
                if let Some(v) = arg.strip_prefix("--domain=") {
                    config.domains = Some(v.split(',').map(|s| s.trim().to_string()).collect());
                } else if let Some(v) = arg.strip_prefix("--level=") {
                    config.min_level = Some(v.to_string());
                } else if let Some(v) = arg.strip_prefix("--event=") {
                    config.events = Some(v.split(',').map(|s| s.trim().to_string()).collect());
                } else if let Some(v) = arg.strip_prefix("--strategy=") {
                    config.strategy_id = Some(v.to_string());
                } else if arg == "--json" {
                    as_json = true;
                }
            }
            cmd_filter(&path, &config, as_json);
        }
        "replay" => cmd_replay(&path),
        "audit" => cmd_audit(&path),
        "slice" => {
            if args.len() < 5 {
                eprintln!("Usage: logparse slice <file> <start_ts> <end_ts>");
                std::process::exit(1);
            }
            cmd_slice(&path, &args[3], &args[4]);
        }
        "agent" => cmd_agent(&path),
        _ => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
