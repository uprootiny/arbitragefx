//! Automated hypothesis ledger updater.
//!
//! Reads a BacktestResult JSON or WalkForwardResult JSON, computes Bayesian
//! updates for each hypothesis, and writes an updated hypothesis_ledger.edn.
//!
//! Usage:
//!   cargo run --bin update_ledger -- out/walk_forward/report.json [--dataset-id LABEL] [--regime REGIME]

use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct StrategyResult {
    id: String,
    pnl: f64,
    equity_pnl: f64,
    equity: f64,
    friction: f64,
    max_drawdown: f64,
    trades: u64,
    wins: u64,
    losses: u64,
    fills: u64,
}

#[derive(Debug, Deserialize)]
struct BacktestResult {
    total_pnl: f64,
    max_drawdown: f64,
    buy_hold_pnl: f64,
    strategies: Vec<StrategyResult>,
    config_hash: Option<String>,
    candle_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WalkForwardSummary {
    id: String,
    train_mean_pnl: f64,
    test_mean_pnl: f64,
    overfit_ratio: f64,
    test_positive_windows: usize,
    total_windows: usize,
    p_value: f64,
    survives_correction: bool,
}

#[derive(Debug, Deserialize)]
struct WalkForwardResult {
    summaries: Vec<WalkForwardSummary>,
    num_strategies: usize,
    num_windows: usize,
    num_comparisons: usize,
    alpha: f64,
    correction_method: String,
}

/// Bayesian truth value: (strength, confidence)
#[derive(Debug, Clone)]
struct Stv {
    strength: f64,
    confidence: f64,
}

impl Stv {
    /// Bayesian update: blend prior with new observation.
    fn update(&self, observation_strength: f64, evidence_weight: f64) -> Self {
        let new_confidence = 1.0 - (1.0 - self.confidence) * (1.0 - evidence_weight);
        // Weighted blend of prior and observation
        let w = evidence_weight / new_confidence.max(1e-9);
        let new_strength = self.strength * (1.0 - w) + observation_strength * w;
        Stv {
            strength: new_strength.clamp(0.0, 1.0),
            confidence: new_confidence.clamp(0.0, 1.0),
        }
    }
}

/// Hypothesis evaluation from backtest data.
struct HypothesisUpdate {
    id: String,
    observation: String,
    observation_strength: f64,
    evidence_weight: f64,
    supports: String,
}

fn evaluate_backtest(result: &BacktestResult, _dataset_id: &str) -> Vec<HypothesisUpdate> {
    let mut updates = Vec::new();
    let positive_count = result.strategies.iter().filter(|s| s.pnl > 0.0).count();
    let total = result.strategies.len();
    let best = result
        .strategies
        .iter()
        .max_by(|a, b| a.pnl.partial_cmp(&b.pnl).unwrap());
    let _worst = result
        .strategies
        .iter()
        .min_by(|a, b| a.pnl.partial_cmp(&b.pnl).unwrap());
    let max_dd = result
        .strategies
        .iter()
        .map(|s| s.max_drawdown)
        .fold(0.0, f64::max);
    let avg_friction =
        result.strategies.iter().map(|s| s.friction).sum::<f64>() / total.max(1) as f64;
    let net_positive = result
        .strategies
        .iter()
        .filter(|s| s.equity_pnl > 0.0)
        .count();

    // H001: Momentum generates raw alpha
    let alpha_frac = positive_count as f64 / total as f64;
    updates.push(HypothesisUpdate {
        id: "H001".into(),
        observation: format!(
            "{}/{} strategies positive raw PnL. Best: {:.2}",
            positive_count,
            total,
            best.map(|b| b.pnl).unwrap_or(0.0)
        ),
        observation_strength: alpha_frac,
        evidence_weight: 0.15,
        supports: if positive_count > total / 2 {
            ":partial"
        } else if positive_count == 0 {
            "false"
        } else {
            ":partial"
        }
        .into(),
    });

    // H002: Friction dominates alpha
    let friction_dominates = net_positive < total / 3;
    updates.push(HypothesisUpdate {
        id: "H002".into(),
        observation: format!(
            "{}/{} net positive after friction. Avg friction: {:.2}",
            net_positive, total, avg_friction
        ),
        observation_strength: if friction_dominates { 0.85 } else { 0.4 },
        evidence_weight: 0.15,
        supports: if friction_dominates {
            "true"
        } else {
            ":partial"
        }
        .into(),
    });

    // H003: Position sizing limits drawdown
    let dd_under_2pct = max_dd <= 0.02;
    updates.push(HypothesisUpdate {
        id: "H003".into(),
        observation: format!("Max DD: {:.2}% across all strategies", max_dd * 100.0),
        observation_strength: if dd_under_2pct { 0.95 } else { 0.5 },
        evidence_weight: 0.15,
        supports: if dd_under_2pct { "true" } else { "false" }.into(),
    });

    // H007: No strategy consistently beats no-trade
    let beats_notrade = net_positive > 0;
    updates.push(HypothesisUpdate {
        id: "H007".into(),
        observation: format!("{}/{} strategies beat no-trade", net_positive, total),
        observation_strength: if beats_notrade { 0.5 } else { 0.9 },
        evidence_weight: 0.15,
        supports: if beats_notrade { ":partial" } else { "true" }.into(),
    });

    // H008: Trade frequency drives friction
    let low_freq = result.strategies.iter().min_by_key(|s| s.trades);
    let high_freq = result.strategies.iter().max_by_key(|s| s.trades);
    if let (Some(lo), Some(hi)) = (low_freq, high_freq) {
        let trade_ratio = hi.trades as f64 / lo.trades.max(1) as f64;
        let friction_ratio = hi.friction / lo.friction.max(1e-9);
        updates.push(HypothesisUpdate {
            id: "H008".into(),
            observation: format!("{}: {} trades ${:.2} friction vs {}: {} trades ${:.2} friction ({}x trades -> {:.1}x friction)",
                lo.id, lo.trades, lo.friction, hi.id, hi.trades, hi.friction, trade_ratio, friction_ratio),
            observation_strength: if friction_ratio > trade_ratio { 0.95 } else { 0.7 },
            evidence_weight: 0.15,
            supports: "true".into(),
        });
    }

    // H009: Capital preservation edge
    if result.buy_hold_pnl < 0.0 {
        let preservation = 1.0 - max_dd; // ~0.99 is great
        updates.push(HypothesisUpdate {
            id: "H009".into(),
            observation: format!(
                "Buy-hold: {:.2}, system max DD: {:.2}%. Preservation: {:.1}%",
                result.buy_hold_pnl,
                max_dd * 100.0,
                preservation * 100.0
            ),
            observation_strength: if preservation > 0.97 { 0.9 } else { 0.6 },
            evidence_weight: 0.15,
            supports: "true".into(),
        });
    }

    updates
}

fn evaluate_walk_forward(wf: &WalkForwardResult) -> Vec<HypothesisUpdate> {
    let mut updates = Vec::new();
    let survivors = wf
        .summaries
        .iter()
        .filter(|s| s.survives_correction)
        .count();
    let total = wf.summaries.len();

    // H001: Raw alpha — does it survive walk-forward?
    let test_positive = wf
        .summaries
        .iter()
        .filter(|s| s.test_mean_pnl > 0.0)
        .count();
    updates.push(HypothesisUpdate {
        id: "H001".into(),
        observation: format!(
            "{}/{} strategies positive in test period, {}/{} survive Bonferroni",
            test_positive, total, survivors, total
        ),
        observation_strength: test_positive as f64 / total as f64,
        evidence_weight: 0.20, // Walk-forward evidence is stronger
        supports: if survivors > 0 { ":partial" } else { "false" }.into(),
    });

    // H007: No consistent alpha — walk-forward verdict
    updates.push(HypothesisUpdate {
        id: "H007".into(),
        observation: format!(
            "{}/{} survive correction ({}). {} comparisons at alpha={}",
            survivors, total, wf.correction_method, wf.num_comparisons, wf.alpha
        ),
        observation_strength: if survivors == 0 { 0.95 } else { 0.3 },
        evidence_weight: 0.25,
        supports: if survivors == 0 { "true" } else { ":partial" }.into(),
    });

    // Overfit detection
    let mean_overfit = wf.summaries.iter().map(|s| s.overfit_ratio).sum::<f64>() / total as f64;
    updates.push(HypothesisUpdate {
        id: "H002".into(),
        observation: format!(
            "Mean overfit ratio (test/train): {:.2}. {} survive after correction.",
            mean_overfit, survivors
        ),
        observation_strength: if mean_overfit < 0.5 { 0.9 } else { 0.5 },
        evidence_weight: 0.20,
        supports: if mean_overfit < 0.5 {
            "true"
        } else {
            ":partial"
        }
        .into(),
    });

    updates
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("out/walk_forward/report.json");
    let dataset_id = args
        .iter()
        .position(|a| a == "--dataset-id")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("auto");

    let content = fs::read_to_string(json_path).expect("failed to read JSON");

    // Try to parse as WalkForwardResult first, then BacktestResult
    let updates = if let Ok(wf) = serde_json::from_str::<WalkForwardResult>(&content) {
        println!(
            "Parsed walk-forward result: {} strategies, {} windows",
            wf.num_strategies, wf.num_windows
        );
        evaluate_walk_forward(&wf)
    } else if let Ok(bt) = serde_json::from_str::<BacktestResult>(&content) {
        println!(
            "Parsed backtest result: {} strategies, {} candles",
            bt.strategies.len(),
            bt.candle_count.unwrap_or(0)
        );
        evaluate_backtest(&bt, dataset_id)
    } else {
        eprintln!(
            "Could not parse {} as BacktestResult or WalkForwardResult",
            json_path
        );
        std::process::exit(1);
    };

    println!();
    println!("Hypothesis Updates");
    println!("==================");
    for u in &updates {
        println!();
        println!(
            "  {} [supports: {}] (weight: {:.2})",
            u.id, u.supports, u.evidence_weight
        );
        println!("  observation: {}", u.observation);
        println!("  observation_strength: {:.2}", u.observation_strength);
    }

    // Read current ledger and apply updates
    let ledger_path = "hypothesis_ledger.edn";
    if let Ok(ledger) = fs::read_to_string(ledger_path) {
        println!();
        println!("Current ledger truth values:");
        let mut current_stvs: Vec<(String, Stv)> = Vec::new();
        let mut current_id = String::new();
        for line in ledger.lines() {
            let t = line.trim();
            if t.contains(":id \"H") && !t.contains(":id :") {
                let id_start = t.find(":id \"").unwrap_or(0) + 5;
                let id_end = t[id_start..]
                    .find('"')
                    .map(|i| id_start + i)
                    .unwrap_or(t.len());
                current_id = t[id_start..id_end].to_string();
            }
            if t.starts_with(":current (stv") {
                let inner = t.trim_start_matches(":current (stv ").trim_end_matches(')');
                let parts: Vec<&str> = inner.split_whitespace().collect();
                if parts.len() >= 2 {
                    let s: f64 = parts[0].parse().unwrap_or(0.0);
                    let c: f64 = parts[1].parse().unwrap_or(0.0);
                    current_stvs.push((
                        current_id.clone(),
                        Stv {
                            strength: s,
                            confidence: c,
                        },
                    ));
                }
            }
        }

        println!();
        println!(
            "  {:<8} {:>12} {:>12} {:>12} {:>12}",
            "ID", "Old S", "New S", "Old C", "New C"
        );
        println!("  {}", "-".repeat(60));
        for (id, stv) in &current_stvs {
            let matching_updates: Vec<_> = updates.iter().filter(|u| u.id == *id).collect();
            if matching_updates.is_empty() {
                println!(
                    "  {:<8} {:>12.4} {:>12} {:>12.4} {:>12}",
                    id, stv.strength, "(no update)", stv.confidence, ""
                );
            } else {
                let mut current = stv.clone();
                for u in &matching_updates {
                    current = current.update(u.observation_strength, u.evidence_weight);
                }
                let s_delta = current.strength - stv.strength;
                let c_delta = current.confidence - stv.confidence;
                println!(
                    "  {:<8} {:>12.4} {:>12.4} {:>12.4} {:>12.4}  (S {:+.4}, C {:+.4})",
                    id,
                    stv.strength,
                    current.strength,
                    stv.confidence,
                    current.confidence,
                    s_delta,
                    c_delta
                );
            }
        }

        println!();
        println!("To apply these updates, re-run with --apply flag (not yet implemented).");
        println!("Manual review recommended before modifying the ledger.");
    }
}
