//! Extended parameter sweep with real data and multiple hypotheses.
//!
//! Usage: cargo run --release --bin sweep -- [data.csv]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, Fill, MarketState, StrategyInstance};
use arbitragefx::strategy::{Action, MarketAux};

/// Hypothesis being tested
#[derive(Debug, Clone)]
struct Hypothesis {
    name: &'static str,
    // Strategy parameters
    entry_threshold: f64,
    exit_threshold: f64,
    edge_scale: f64,
    edge_hurdle: f64,
    // Risk parameters
    max_position_pct: f64,
    stop_loss: f64,
    take_profit: f64,
    // Execution parameters
    position_size: f64,  // Base qty per trade
    fee_rate: f64,
    slippage_k: f64,
}

impl Hypothesis {
    fn baseline() -> Self {
        Self {
            name: "baseline",
            entry_threshold: 1.2,
            exit_threshold: 0.4,
            edge_scale: 0.0025,
            edge_hurdle: 0.003,
            max_position_pct: 0.05,
            stop_loss: 0.004,
            take_profit: 0.006,
            position_size: 0.001,
            fee_rate: 0.001,
            slippage_k: 0.0008,
        }
    }

    fn variants() -> Vec<Self> {
        vec![
            // Baseline
            Self::baseline(),

            // === POSITION SIZING ===
            Self { name: "pos_tiny", position_size: 0.0005, ..Self::baseline() },
            Self { name: "pos_small", position_size: 0.002, ..Self::baseline() },
            Self { name: "pos_medium", position_size: 0.005, ..Self::baseline() },
            Self { name: "pos_large", position_size: 0.01, ..Self::baseline() },

            // === STOP/PROFIT LEVELS ===
            Self { name: "stop_tight", stop_loss: 0.002, take_profit: 0.003, ..Self::baseline() },
            Self { name: "stop_balanced", stop_loss: 0.004, take_profit: 0.006, ..Self::baseline() },
            Self { name: "stop_wide", stop_loss: 0.008, take_profit: 0.012, ..Self::baseline() },
            Self { name: "stop_asym_up", stop_loss: 0.003, take_profit: 0.009, ..Self::baseline() },  // 3:1 reward
            Self { name: "stop_asym_dn", stop_loss: 0.006, take_profit: 0.004, ..Self::baseline() },  // quick profit

            // === ENTRY SELECTIVITY ===
            Self { name: "entry_loose", entry_threshold: 0.8, edge_hurdle: 0.001, ..Self::baseline() },
            Self { name: "entry_normal", entry_threshold: 1.2, edge_hurdle: 0.003, ..Self::baseline() },
            Self { name: "entry_strict", entry_threshold: 1.8, edge_hurdle: 0.005, ..Self::baseline() },
            Self { name: "entry_vstrict", entry_threshold: 2.5, edge_hurdle: 0.008, ..Self::baseline() },

            // === EXIT TIMING ===
            Self { name: "exit_quick", exit_threshold: 0.2, ..Self::baseline() },
            Self { name: "exit_patient", exit_threshold: 0.6, ..Self::baseline() },
            Self { name: "exit_vpatient", exit_threshold: 0.8, ..Self::baseline() },

            // === FEE SCENARIOS ===
            Self { name: "fee_zero", fee_rate: 0.0, slippage_k: 0.0, ..Self::baseline() },
            Self { name: "fee_maker", fee_rate: 0.0002, slippage_k: 0.0002, ..Self::baseline() },
            Self { name: "fee_vip", fee_rate: 0.0004, slippage_k: 0.0004, ..Self::baseline() },
            Self { name: "fee_taker", fee_rate: 0.001, slippage_k: 0.0008, ..Self::baseline() },

            // === COMBINED OPTIMIZED ===
            // Maker fees + strict entry + balanced stops
            Self {
                name: "opt_maker_strict",
                entry_threshold: 1.8,
                exit_threshold: 0.4,
                edge_hurdle: 0.004,
                stop_loss: 0.004,
                take_profit: 0.006,
                position_size: 0.002,
                fee_rate: 0.0002,
                slippage_k: 0.0002,
                ..Self::baseline()
            },
            // Maker fees + loose entry + tight stops (scalping)
            Self {
                name: "opt_scalp",
                entry_threshold: 0.8,
                exit_threshold: 0.3,
                edge_hurdle: 0.001,
                stop_loss: 0.002,
                take_profit: 0.003,
                position_size: 0.003,
                fee_rate: 0.0002,
                slippage_k: 0.0002,
                ..Self::baseline()
            },
            // Zero fees + asymmetric risk (swing trading)
            Self {
                name: "opt_swing",
                entry_threshold: 2.0,
                exit_threshold: 0.5,
                edge_hurdle: 0.006,
                stop_loss: 0.003,
                take_profit: 0.012,  // 4:1 reward
                position_size: 0.002,
                fee_rate: 0.0,
                slippage_k: 0.0,
                ..Self::baseline()
            },
            // Balanced optimal for taker fees
            Self {
                name: "opt_taker",
                entry_threshold: 1.5,
                exit_threshold: 0.4,
                edge_hurdle: 0.005,
                stop_loss: 0.005,
                take_profit: 0.008,
                position_size: 0.002,
                fee_rate: 0.001,
                slippage_k: 0.0008,
                ..Self::baseline()
            },

            // === TIMEFRAME-SPECIFIC ===
            // For longer timeframes (15m, 1h): very selective, wider stops
            Self {
                name: "tf_long_maker",
                entry_threshold: 2.5,
                exit_threshold: 0.6,
                edge_hurdle: 0.008,
                stop_loss: 0.008,
                take_profit: 0.015,
                position_size: 0.001,
                fee_rate: 0.0002,
                slippage_k: 0.0002,
                ..Self::baseline()
            },
            Self {
                name: "tf_long_taker",
                entry_threshold: 2.5,
                exit_threshold: 0.6,
                edge_hurdle: 0.008,
                stop_loss: 0.008,
                take_profit: 0.015,
                position_size: 0.001,
                fee_rate: 0.001,
                slippage_k: 0.0008,
                ..Self::baseline()
            },
            // For short timeframes (1m, 5m): quick scalping
            Self {
                name: "tf_short_maker",
                entry_threshold: 1.0,
                exit_threshold: 0.3,
                edge_hurdle: 0.002,
                stop_loss: 0.003,
                take_profit: 0.004,
                position_size: 0.001,
                fee_rate: 0.0002,
                slippage_k: 0.0002,
                ..Self::baseline()
            },
            Self {
                name: "tf_short_taker",
                entry_threshold: 1.0,
                exit_threshold: 0.3,
                edge_hurdle: 0.002,
                stop_loss: 0.003,
                take_profit: 0.004,
                position_size: 0.001,
                fee_rate: 0.001,
                slippage_k: 0.0008,
                ..Self::baseline()
            },

            // === EDGE HURDLE TESTS ===
            Self { name: "edge_low", edge_hurdle: 0.001, ..Self::baseline() },
            Self { name: "edge_mid", edge_hurdle: 0.004, ..Self::baseline() },
            Self { name: "edge_high", edge_hurdle: 0.008, ..Self::baseline() },
            Self { name: "edge_vhigh", edge_hurdle: 0.012, ..Self::baseline() },
        ]
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TrialResult {
    hypothesis: String,
    pnl: f64,
    equity_delta: f64,
    max_drawdown: f64,
    trades: u64,
    wins: u64,
    losses: u64,
    friction: f64,
    runtime_ms: u64,
}

fn run_hypothesis(h: &Hypothesis, rows: &[CsvRow]) -> TrialResult {
    let start = Instant::now();

    let mut cfg = Config::from_env();
    cfg.entry_threshold = h.entry_threshold;
    cfg.exit_threshold = h.exit_threshold;
    cfg.edge_scale = h.edge_scale;
    cfg.edge_hurdle = h.edge_hurdle;
    cfg.max_position_pct = h.max_position_pct;
    cfg.stop_loss = h.stop_loss;
    cfg.take_profit = h.take_profit;

    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();

    let initial_cash = 1000.0;
    let mut friction = 0.0;

    for row in rows {
        let candle = arbitragefx::exchange::Candle {
            ts: row.ts,
            o: row.o,
            h: row.h,
            l: row.l,
            c: row.c,
            v: row.v,
        };
        market.on_candle(candle);
        market.update_aux(
            &cfg.symbol,
            MarketAux {
                funding_rate: row.funding,
                borrow_rate: row.borrow,
                liquidation_score: row.liq,
                stable_depeg: row.depeg,
                fetch_ts: row.ts,
                has_funding: row.funding != 0.0,
                has_borrow: row.borrow != 0.0,
                has_liquidations: row.liq != 0.0,
                has_depeg: row.depeg != 0.0,
            },
        );

        for inst in strategies.iter_mut() {
            let view = market.view(&cfg.symbol);
            let action = inst.strategy.update(view, &mut inst.state);
            let guarded = risk.apply_with_price(&inst.state, action, row.ts, row.c);

            let fill = match guarded {
                Action::Hold => None,
                Action::Close => {
                    let qty = -inst.state.portfolio.position;
                    if qty.abs() > 1e-9 { Some((qty, row.c)) } else { None }
                }
                Action::Buy { qty: _ } => Some((h.position_size, row.c)),
                Action::Sell { qty: _ } => Some((-h.position_size, row.c)),
            };

            if let Some((qty, price)) = fill {
                // Calculate volatility from high-low range
                let volatility = (row.h - row.l) / row.c;

                // Fill probability simulation (deterministic based on bar index for reproducibility)
                // In reality this would be stochastic, but we want reproducible backtests
                let bar_hash = (row.ts % 100) as f64 / 100.0;
                let is_buy = qty > 0.0;

                // Base fill probability adjusted by volatility and adverse selection
                let base_prob = if h.fee_rate < 0.0005 { 0.7 } else { 0.95 };  // Limit vs market
                let adverse_sel = 0.25;
                let fill_prob = base_prob * (1.0 - adverse_sel * volatility * 10.0).max(0.3);

                // Skip fill if probability check fails (simulates unfilled limit orders)
                if h.fee_rate < 0.0005 && bar_hash > fill_prob {
                    // Limit order didn't fill - no trade this bar
                    continue;
                }

                // Apply slippage (higher in volatile markets)
                let vol_mult = 1.0 + volatility * 2.0;
                let slip = h.slippage_k * qty.abs() / row.v.max(1.0) * vol_mult;
                let fill_price = if is_buy { price * (1.0 + slip) } else { price * (1.0 - slip) };
                let fee = fill_price * qty.abs() * h.fee_rate;
                let slip_cost = (fill_price - price).abs() * qty.abs();
                friction += fee + slip_cost;

                let realized = inst.state.portfolio.apply_fill(Fill {
                    price: fill_price,
                    qty,
                    fee,
                    ts: row.ts,
                });
                inst.state.metrics.pnl += realized;
                if realized > 0.0 {
                    inst.state.metrics.wins += 1;
                } else if realized < 0.0 {
                    inst.state.metrics.losses += 1;
                    inst.state.last_loss_ts = row.ts;
                }
                inst.state.last_trade_ts = row.ts;
            }
            // Mark to market with current price
            metrics.update_with_price(&mut inst.state, row.c);
        }
    }

    // Force close remaining positions
    if let Some(last) = rows.last() {
        for inst in strategies.iter_mut() {
            if inst.state.portfolio.position.abs() > 1e-9 {
                let qty = -inst.state.portfolio.position;
                let fee = last.c * qty.abs() * h.fee_rate;
                friction += fee;
                let realized = inst.state.portfolio.apply_fill(Fill {
                    price: last.c,
                    qty,
                    fee,
                    ts: last.ts,
                });
                inst.state.metrics.pnl += realized;
            }
        }
    }

    let n_strategies = strategies.len() as f64;
    let pnl: f64 = strategies.iter().map(|s| s.state.metrics.pnl).sum::<f64>() / n_strategies;
    let equity_delta: f64 = strategies.iter().map(|s| s.state.portfolio.equity - initial_cash).sum::<f64>() / n_strategies;
    let max_dd = strategies.iter().map(|s| s.state.metrics.max_drawdown).fold(0.0, f64::min);
    let wins: u64 = strategies.iter().map(|s| s.state.metrics.wins).sum();
    let losses: u64 = strategies.iter().map(|s| s.state.metrics.losses).sum();

    TrialResult {
        hypothesis: h.name.to_string(),
        pnl,
        equity_delta,
        max_drawdown: max_dd,
        trades: wins + losses,
        wins,
        losses,
        friction,
        runtime_ms: start.elapsed().as_millis() as u64,
    }
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "data/btc_binance.csv".to_string());

    println!("Loading data from {}...", path);
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open {}: {}", path, e);
            return;
        }
    };

    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') || line.to_lowercase().starts_with("ts,") {
            continue;
        }
        match parse_csv_line(&line) {
            Ok(r) => rows.push(r),
            Err(e) => eprintln!("Bad row: {}", e),
        }
    }

    if rows.is_empty() {
        eprintln!("No data rows");
        return;
    }

    let first_price = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last_price = rows.last().map(|r| r.c).unwrap_or(0.0);
    let buy_hold = last_price - first_price;

    println!("Loaded {} bars", rows.len());
    println!("Price range: {:.2} -> {:.2} (buy-hold: {:.2})", first_price, last_price, buy_hold);
    println!();

    let hypotheses = Hypothesis::variants();
    println!("Running {} hypotheses...", hypotheses.len());
    println!();

    let mut results: Vec<TrialResult> = hypotheses
        .iter()
        .map(|h| run_hypothesis(h, &rows))
        .collect();

    // Sort by equity delta (most profitable first)
    results.sort_by(|a, b| b.equity_delta.partial_cmp(&a.equity_delta).unwrap());

    // Print results table
    println!("{:<20} {:>10} {:>12} {:>10} {:>8} {:>8} {:>10} {:>8}",
             "Hypothesis", "PnL", "Equity Δ", "MaxDD", "Trades", "Win%", "Friction", "ms");
    println!("{}", "-".repeat(96));

    for r in &results {
        let win_pct = if r.trades > 0 { 100.0 * r.wins as f64 / r.trades as f64 } else { 0.0 };
        println!("{:<20} {:>10.2} {:>12.2} {:>10.4} {:>8} {:>7.1}% {:>10.2} {:>8}",
                 r.hypothesis, r.pnl, r.equity_delta, r.max_drawdown, r.trades, win_pct, r.friction, r.runtime_ms);
    }

    println!();
    println!("Best: {} (equity Δ: {:.2})", results[0].hypothesis, results[0].equity_delta);
    println!("Worst: {} (equity Δ: {:.2})", results.last().unwrap().hypothesis, results.last().unwrap().equity_delta);

    // Categorized analysis
    println!();
    println!("=== Analysis by Category ===");

    let baseline = results.iter().find(|r| r.hypothesis == "baseline");
    let zero_fees = results.iter().find(|r| r.hypothesis == "fee_zero");
    let maker_fees = results.iter().find(|r| r.hypothesis == "fee_maker");

    if let (Some(base), Some(zero)) = (baseline, zero_fees) {
        let friction_impact = zero.equity_delta - base.equity_delta;
        println!("\nFRICTION IMPACT:");
        println!("  Zero fees: {:.2} equity", zero.equity_delta);
        println!("  Baseline:  {:.2} equity", base.equity_delta);
        println!("  Impact:    {:.2} ({:.1}% drag)", friction_impact, 100.0 * friction_impact.abs() / zero.equity_delta.abs().max(0.01));
    }

    if let Some(maker) = maker_fees {
        println!("\nMAKER vs TAKER:");
        println!("  Maker fees: {:.2} equity, {:.1}% win", maker.equity_delta,
                 if maker.trades > 0 { 100.0 * maker.wins as f64 / maker.trades as f64 } else { 0.0 });
        if let Some(base) = baseline {
            println!("  Taker fees: {:.2} equity, {:.1}% win", base.equity_delta,
                     if base.trades > 0 { 100.0 * base.wins as f64 / base.trades as f64 } else { 0.0 });
        }
    }

    // Position sizing analysis
    let pos_results: Vec<_> = results.iter()
        .filter(|r| r.hypothesis.starts_with("pos_"))
        .collect();
    if !pos_results.is_empty() {
        println!("\nPOSITION SIZING:");
        for r in pos_results {
            let win_pct = if r.trades > 0 { 100.0 * r.wins as f64 / r.trades as f64 } else { 0.0 };
            println!("  {:<12} equity={:>8.2}  trades={:>4}  win={:.1}%",
                     r.hypothesis, r.equity_delta, r.trades, win_pct);
        }
    }

    // Entry selectivity analysis
    let entry_results: Vec<_> = results.iter()
        .filter(|r| r.hypothesis.starts_with("entry_"))
        .collect();
    if !entry_results.is_empty() {
        println!("\nENTRY SELECTIVITY:");
        for r in entry_results {
            let win_pct = if r.trades > 0 { 100.0 * r.wins as f64 / r.trades as f64 } else { 0.0 };
            println!("  {:<12} equity={:>8.2}  trades={:>4}  win={:.1}%",
                     r.hypothesis, r.equity_delta, r.trades, win_pct);
        }
    }

    // Stop/profit analysis
    let stop_results: Vec<_> = results.iter()
        .filter(|r| r.hypothesis.starts_with("stop_"))
        .collect();
    if !stop_results.is_empty() {
        println!("\nSTOP/PROFIT LEVELS:");
        for r in stop_results {
            let win_pct = if r.trades > 0 { 100.0 * r.wins as f64 / r.trades as f64 } else { 0.0 };
            println!("  {:<12} equity={:>8.2}  trades={:>4}  win={:.1}%  maxdd={:.2}%",
                     r.hypothesis, r.equity_delta, r.trades, win_pct, r.max_drawdown * 100.0);
        }
    }

    // Optimized combinations
    let opt_results: Vec<_> = results.iter()
        .filter(|r| r.hypothesis.starts_with("opt_"))
        .collect();
    if !opt_results.is_empty() {
        println!("\nOPTIMIZED COMBINATIONS:");
        for r in opt_results {
            let win_pct = if r.trades > 0 { 100.0 * r.wins as f64 / r.trades as f64 } else { 0.0 };
            let sharpe_proxy = if r.max_drawdown.abs() > 0.001 {
                r.equity_delta / (r.max_drawdown.abs() * 1000.0)
            } else {
                r.equity_delta
            };
            println!("  {:<16} equity={:>8.2}  trades={:>4}  win={:.1}%  sharpe~={:.2}",
                     r.hypothesis, r.equity_delta, r.trades, win_pct, sharpe_proxy);
        }
    }

    // Summary statistics
    println!();
    println!("=== Summary Statistics ===");
    let profitable: Vec<_> = results.iter().filter(|r| r.equity_delta > 0.0).collect();
    let total = results.len();
    println!("Profitable hypotheses: {}/{} ({:.1}%)", profitable.len(), total, 100.0 * profitable.len() as f64 / total as f64);

    let avg_equity: f64 = results.iter().map(|r| r.equity_delta).sum::<f64>() / total as f64;
    let avg_trades: f64 = results.iter().map(|r| r.trades as f64).sum::<f64>() / total as f64;
    let avg_winrate: f64 = results.iter()
        .filter(|r| r.trades > 0)
        .map(|r| r.wins as f64 / r.trades as f64)
        .sum::<f64>() / results.iter().filter(|r| r.trades > 0).count().max(1) as f64;

    println!("Average equity delta: {:.2}", avg_equity);
    println!("Average trades: {:.0}", avg_trades);
    println!("Average win rate: {:.1}%", avg_winrate * 100.0);

    // Best per category
    println!();
    println!("=== Best Per Category ===");
    if let Some(best) = results.iter().max_by(|a, b| a.equity_delta.partial_cmp(&b.equity_delta).unwrap()) {
        println!("Best equity:    {} ({:.2})", best.hypothesis, best.equity_delta);
    }
    if let Some(best) = results.iter().filter(|r| r.trades > 10).max_by(|a, b| {
        let a_wr = a.wins as f64 / a.trades as f64;
        let b_wr = b.wins as f64 / b.trades as f64;
        a_wr.partial_cmp(&b_wr).unwrap()
    }) {
        println!("Best win rate:  {} ({:.1}%, {} trades)", best.hypothesis,
                 100.0 * best.wins as f64 / best.trades as f64, best.trades);
    }
    if let Some(best) = results.iter().filter(|r| r.max_drawdown < 0.0).min_by(|a, b| {
        a.max_drawdown.abs().partial_cmp(&b.max_drawdown.abs()).unwrap()
    }) {
        println!("Lowest drawdown: {} ({:.2}%)", best.hypothesis, best.max_drawdown * 100.0);
    }
}
