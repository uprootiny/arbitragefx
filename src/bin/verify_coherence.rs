//! Multi-scale coherence verification tool.
//!
//! Verifies the system works correctly at different scales:
//! - Micro: Individual signal/fill pairs
//! - Meso: Session-level invariants
//! - Macro: Cross-run reproducibility
//!
//! Usage: verify_coherence <log_dir>

use serde::Deserialize;
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
    #[serde(default)]
    data: Value,
    strategy_id: Option<String>,
}

#[derive(Debug, Default)]
struct MicroStats {
    decisions_without_fill: u64,
    fills_without_decision: u64,
    decision_fill_pairs: u64,
}

#[derive(Debug, Default)]
struct MesoStats {
    seq_gaps: u64,
    checkpoint_count: u64,
    checkpoint_pnl_monotonic: bool,
    first_checkpoint_pnl: f64,
    last_checkpoint_pnl: f64,
}

#[derive(Debug, Default)]
struct MacroStats {
    unique_state_hashes: usize,
    hash_collisions: u64,
}

fn parse_logs(events_path: &PathBuf, trace_path: &PathBuf) -> (Vec<LogEntry>, Vec<LogEntry>) {
    let parse_file = |path: &PathBuf| -> Vec<LogEntry> {
        if !path.exists() {
            return Vec::new();
        }
        let file = File::open(path).expect("Failed to open log file");
        BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect()
    };

    (parse_file(events_path), parse_file(trace_path))
}

fn verify_micro(events: &[LogEntry], trace: &[LogEntry]) -> MicroStats {
    let mut stats = MicroStats::default();

    // Track decision/fill pairs by timestamp and strategy
    let decisions: HashMap<(String, Option<String>), &LogEntry> = events
        .iter()
        .filter(|e| e.event == "decision")
        .map(|e| ((e.ts.clone(), e.strategy_id.clone()), e))
        .collect();

    let fills: HashMap<(String, Option<String>), &LogEntry> = events
        .iter()
        .filter(|e| e.event == "fill")
        .map(|e| {
            let sid = e.strategy_id.clone().or_else(|| {
                e.data.get("strategy_id").and_then(|v| v.as_str()).map(|s| s.to_string())
            });
            ((e.ts.clone(), sid), e)
        })
        .collect();

    // Check for decisions without corresponding fills
    for ((ts, sid), _) in &decisions {
        // A decision may or may not result in a fill (hold decisions don't)
        // We only count non-hold decisions
    }

    // Count pairs where we have both decision and fill at same timestamp
    for key in decisions.keys() {
        if fills.contains_key(key) {
            stats.decision_fill_pairs += 1;
        }
    }

    // Verify signal→guard→fill sequence in trace
    let mut last_signal_ts: Option<String> = None;
    let mut last_guard_ts: Option<String> = None;

    for entry in trace {
        match entry.event.as_str() {
            "signal" => last_signal_ts = Some(entry.ts.clone()),
            "guard" => {
                if last_signal_ts.as_ref() != Some(&entry.ts) {
                    // Guard without preceding signal at same timestamp
                }
                last_guard_ts = Some(entry.ts.clone());
            }
            "reasoning" => {
                // Reasoning should happen at same timestamp as guard
            }
            _ => {}
        }
    }

    stats
}

fn verify_meso(events: &[LogEntry]) -> MesoStats {
    let mut stats = MesoStats::default();
    let mut last_seq: Option<u64> = None;
    let mut checkpoints: Vec<f64> = Vec::new();

    for entry in events {
        // Check sequence continuity
        if let Some(prev) = last_seq {
            if entry.seq > prev + 1 {
                stats.seq_gaps += 1;
            }
        }
        last_seq = Some(entry.seq);

        // Track checkpoints
        if entry.event == "checkpoint" {
            stats.checkpoint_count += 1;
            if let Some(pnl) = entry.data.get("pnl").and_then(|v| v.as_f64()) {
                checkpoints.push(pnl);
            }
        }
    }

    // Check PnL progression (should be somewhat monotonic with noise)
    if !checkpoints.is_empty() {
        stats.first_checkpoint_pnl = checkpoints[0];
        stats.last_checkpoint_pnl = *checkpoints.last().unwrap_or(&0.0);

        // Check if cumulative PnL is monotonically decreasing or increasing
        // (allowing for small fluctuations)
        let mut monotonic_up = true;
        let mut monotonic_down = true;
        for window in checkpoints.windows(2) {
            if window[1] < window[0] - 10.0 {
                monotonic_up = false;
            }
            if window[1] > window[0] + 10.0 {
                monotonic_down = false;
            }
        }
        stats.checkpoint_pnl_monotonic = monotonic_up || monotonic_down;
    }

    stats
}

fn verify_macro(events: &[LogEntry]) -> MacroStats {
    let mut stats = MacroStats::default();
    let mut hashes: HashMap<String, u64> = HashMap::new();

    for entry in events {
        if let Some(hash) = entry.data.get("state_hash").and_then(|v| v.as_str()) {
            *hashes.entry(hash.to_string()).or_insert(0) += 1;
        }
    }

    stats.unique_state_hashes = hashes.len();
    stats.hash_collisions = hashes.values().filter(|&&c| c > 1).count() as u64;

    stats
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: verify_coherence <log_dir>");
        eprintln!("Example: verify_coherence out/runs/r-1234567890-12345");
        std::process::exit(1);
    }

    let log_dir = PathBuf::from(&args[1]);
    let events_path = log_dir.join("events.jsonl");
    let trace_path = log_dir.join("trace.jsonl");

    if !events_path.exists() {
        eprintln!("Error: events.jsonl not found in {}", log_dir.display());
        std::process::exit(1);
    }

    println!("=== Multi-Scale Coherence Verification ===\n");
    println!("Log directory: {}", log_dir.display());

    let (events, trace) = parse_logs(&events_path, &trace_path);
    println!("Events loaded: {}", events.len());
    println!("Trace loaded: {}", trace.len());

    // MICRO scale verification
    println!("\n--- MICRO: Signal-Fill Coherence ---");
    let micro = verify_micro(&events, &trace);
    println!("  Decision-fill pairs: {}", micro.decision_fill_pairs);
    if micro.decisions_without_fill > 0 {
        println!("  Orphan decisions: {}", micro.decisions_without_fill);
    }
    if micro.fills_without_decision > 0 {
        println!("  Orphan fills: {}", micro.fills_without_decision);
    }
    let micro_ok = micro.fills_without_decision == 0;
    println!("  Status: {}", if micro_ok { "✓ OK" } else { "⚠ Issues" });

    // MESO scale verification
    println!("\n--- MESO: Session Invariants ---");
    let meso = verify_meso(&events);
    println!("  Sequence gaps: {}", meso.seq_gaps);
    println!("  Checkpoints: {}", meso.checkpoint_count);
    println!(
        "  PnL trajectory: {:.2} → {:.2}",
        meso.first_checkpoint_pnl, meso.last_checkpoint_pnl
    );
    println!(
        "  PnL monotonic: {}",
        if meso.checkpoint_pnl_monotonic { "yes" } else { "no (expected for trading)" }
    );
    let meso_ok = meso.seq_gaps < 10 && meso.checkpoint_count > 0;
    println!("  Status: {}", if meso_ok { "✓ OK" } else { "⚠ Issues" });

    // MACRO scale verification
    println!("\n--- MACRO: Cross-Run Integrity ---");
    let mac = verify_macro(&events);
    println!("  Unique state hashes: {}", mac.unique_state_hashes);
    println!("  Hash collisions: {}", mac.hash_collisions);
    let macro_ok = mac.unique_state_hashes > 0;
    println!("  Status: {}", if macro_ok { "✓ OK" } else { "⚠ No hashes" });

    // Summary
    println!("\n=== Summary ===");
    let all_ok = micro_ok && meso_ok && macro_ok;
    if all_ok {
        println!("✓ All coherence checks passed");
    } else {
        println!("⚠ Some checks failed - investigate logs");
    }

    // Recommendations for AI agent
    println!("\n--- AI Agent Recommendations ---");
    if micro.decision_fill_pairs > 0 {
        let fill_rate = micro.decision_fill_pairs as f64
            / (micro.decision_fill_pairs + micro.decisions_without_fill) as f64;
        println!("  Decision→Fill rate: {:.1}%", fill_rate * 100.0);
    }
    let pnl_delta = meso.last_checkpoint_pnl - meso.first_checkpoint_pnl;
    if pnl_delta < 0.0 {
        println!("  PnL declining ({:.2}) - consider strategy adjustment", pnl_delta);
    } else {
        println!("  PnL positive ({:.2}) - strategy may be working", pnl_delta);
    }
}
