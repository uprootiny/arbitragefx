//! Logged backtest runner - emits structured logs at multiple scales.
//!
//! Usage: LOG_LEVEL=debug cargo run --release --bin backtest_logged -- [data.csv]
//!
//! Log output: out/runs/<run_id>/{events.jsonl, trace.jsonl, metrics.jsonl}
//!
//! Demonstrates the multi-scale logging architecture:
//! - TRACE: Every candle, every indicator update
//! - DEBUG: Every signal, every risk check
//! - INFO: Decisions, fills, checkpoints
//! - WARN: Drift detection, risk guards triggered
//! - ERROR: Failed orders, system issues

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::logging::{
    log, log_audit, log_candle, log_checkpoint, log_decision, log_drift, log_fill,
    log_market_context, log_periodic_summary, log_reasoning, log_risk_check, log_session_summary,
    log_signal, obj, params_hash, v_num, v_str, Domain, Level, StrategySummary,
};
use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, Fill, MarketState, StrategyInstance};
use arbitragefx::strategy::{Action, MarketAux};

fn load_data(path: &str) -> Vec<CsvRow> {
    let file = File::open(path).expect("Failed to open data file");
    let reader = BufReader::new(file);
    let mut rows = Vec::new();

    for line in reader.lines().skip(1) {
        if let Ok(line) = line {
            if let Ok(row) = parse_csv_line(&line) {
                rows.push(row);
            }
        }
    }
    rows
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data_path = args.get(1).map(|s| s.as_str()).unwrap_or("data/btc_1h_180d.csv");

    // Log startup
    log(
        Level::Info,
        Domain::System,
        "startup",
        obj(&[
            ("version", v_str("0.1.0")),
            ("data_path", v_str(data_path)),
            ("config_hash", v_str(&params_hash(data_path))),
        ]),
    );

    let rows = load_data(data_path);
    let n_bars = rows.len();

    log(
        Level::Info,
        Domain::System,
        "data_loaded",
        obj(&[
            ("bars", v_num(n_bars as f64)),
            (
                "price_range",
                v_str(&format!(
                    "{:.2} → {:.2}",
                    rows.first().map(|r| r.c).unwrap_or(0.0),
                    rows.last().map(|r| r.c).unwrap_or(0.0)
                )),
            ),
        ]),
    );

    let cfg = Config::from_env();
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();

    let start = Instant::now();
    let initial_cash = 1000.0;
    let checkpoint_interval = n_bars / 10; // 10 checkpoints across the run
    let summary_interval = n_bars / 20; // 20 periodic summaries

    let mut total_trades = 0u64;
    let mut drift_events = 0u64;
    let mut max_drawdown = 0.0f64;

    for (bar_idx, row) in rows.iter().enumerate() {
        // Log candle at TRACE level
        log_candle(&cfg.symbol, row.ts, row.o, row.h, row.l, row.c, row.v);

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
            let strategy_id = &inst.id;

            // Log market context for AI agent
            let z_momentum = view.indicators.z_momentum;
            let z_vol = view.indicators.z_vol;
            let regime = if z_vol > 1.5 {
                "high_vol"
            } else if z_momentum.abs() > 1.5 {
                "trending"
            } else {
                "normal"
            };

            // Detect distribution drift (simplified)
            let drift_severity = if z_vol > 2.0 || z_momentum.abs() > 2.5 {
                drift_events += 1;
                log_drift(
                    "moderate",
                    0.8,
                    &[("z_vol", z_vol), ("z_momentum", z_momentum)],
                );
                "moderate"
            } else {
                "none"
            };

            log_market_context(
                &cfg.symbol,
                row.c,
                z_momentum,
                z_vol,
                row.funding,
                regime,
                drift_severity,
            );

            // Get action from strategy
            let action = inst.strategy.update(view, &mut inst.state);
            let action_str = match action {
                Action::Hold => "hold",
                Action::Buy { .. } => "buy",
                Action::Sell { .. } => "sell",
                Action::Close => "close",
            };

            // Log signal at DEBUG level
            let score = z_momentum + z_vol * 0.3;
            log_signal(strategy_id, action_str, score, regime);

            // Apply risk guard
            let guarded = risk.apply_with_price(&inst.state, action, row.ts, row.c);
            let guarded_str = match guarded {
                Action::Hold => "hold",
                Action::Buy { .. } => "buy",
                Action::Sell { .. } => "sell",
                Action::Close => "close",
            };

            // Log risk check
            let risk_result = if guarded_str == action_str {
                "passed"
            } else {
                "blocked"
            };
            log_risk_check(
                "position_limit",
                risk_result,
                inst.state.portfolio.position,
                cfg.max_position_pct,
            );

            // Process fill
            let fill = match guarded {
                Action::Hold => None,
                Action::Close => {
                    let qty = -inst.state.portfolio.position;
                    if qty.abs() > 1e-9 {
                        Some((qty, row.c))
                    } else {
                        None
                    }
                }
                Action::Buy { qty } => Some((qty.min(0.001), row.c)),
                Action::Sell { qty } => Some((-qty.min(0.001), row.c)),
            };

            if let Some((qty, price)) = fill {
                let fee = price * qty.abs() * 0.001;

                // Log reasoning chain
                log_reasoning(
                    strategy_id,
                    &[
                        &format!("Signal: {} (score={:.2})", action_str, score),
                        &format!("Risk check: {}", risk_result),
                        &format!("Fill: qty={:.6} @ {:.2}", qty, price),
                    ],
                );

                // Log decision point for AI
                log_decision(
                    strategy_id,
                    guarded_str,
                    &format!("{}. {}", regime, drift_severity),
                    score.abs().min(1.0),
                    &[("hold", 0.0), (action_str, score)],
                    Some(&params_hash(&format!("{}-{}", row.ts, strategy_id))),
                );

                let realized = inst.state.portfolio.apply_fill(Fill {
                    price,
                    qty,
                    fee,
                    ts: row.ts,
                });

                // Log fill
                log_fill(
                    &format!("o-{}", row.ts),
                    strategy_id,
                    price,
                    qty,
                    fee,
                    realized,
                );

                inst.state.metrics.pnl += realized;
                if realized > 0.0 {
                    inst.state.metrics.wins += 1;
                } else if realized < 0.0 {
                    inst.state.metrics.losses += 1;
                }
                total_trades += 1;
            }

            // Mark to market
            metrics.update_with_price(&mut inst.state, row.c);
            max_drawdown = max_drawdown.min(inst.state.metrics.max_drawdown);
        }

        // Periodic summary (every summary_interval bars)
        if summary_interval > 0 && bar_idx > 0 && bar_idx % summary_interval == 0 {
            let summaries: Vec<StrategySummary> = strategies
                .iter()
                .map(|s| StrategySummary {
                    id: s.id.clone(),
                    pnl: s.state.metrics.pnl,
                    position: s.state.portfolio.position,
                    trades: s.state.metrics.wins + s.state.metrics.losses,
                    win_rate: if s.state.metrics.wins + s.state.metrics.losses > 0 {
                        s.state.metrics.wins as f64
                            / (s.state.metrics.wins + s.state.metrics.losses) as f64
                    } else {
                        0.0
                    },
                })
                .collect();

            let total_pnl: f64 = strategies.iter().map(|s| s.state.metrics.pnl).sum();
            log_periodic_summary(300, &summaries, total_pnl, total_trades, drift_events);
        }

        // Checkpoint (every checkpoint_interval bars)
        if checkpoint_interval > 0 && bar_idx > 0 && bar_idx % checkpoint_interval == 0 {
            for inst in &strategies {
                log_checkpoint(
                    &inst.id,
                    &params_hash(&format!("{}-{}-{}", row.ts, inst.id, inst.state.portfolio.equity)),
                    bar_idx as u64,
                    inst.state.metrics.pnl,
                    inst.state.portfolio.position,
                );

                // Audit entry
                log_audit(
                    "checkpoint",
                    &params_hash(&format!("{}", inst.state.portfolio.equity)),
                    &params_hash(&format!("{}", row.ts)),
                    &params_hash(&format!("{}", inst.state.metrics.pnl)),
                );
            }
        }
    }

    // Close remaining positions
    if let Some(last) = rows.last() {
        for inst in strategies.iter_mut() {
            if inst.state.portfolio.position.abs() > 1e-9 {
                let qty = -inst.state.portfolio.position;
                let fee = last.c * qty.abs() * 0.001;
                let realized = inst.state.portfolio.apply_fill(Fill {
                    price: last.c,
                    qty,
                    fee,
                    ts: last.ts,
                });
                inst.state.metrics.pnl += realized;

                log_fill(
                    &format!("o-{}-close", last.ts),
                    &inst.id,
                    last.c,
                    qty,
                    fee,
                    realized,
                );
            }
        }
    }

    let duration = start.elapsed();
    let total_pnl: f64 = strategies.iter().map(|s| s.state.metrics.pnl).sum::<f64>()
        / strategies.len() as f64;
    let total_wins: u64 = strategies.iter().map(|s| s.state.metrics.wins).sum();
    let total_losses: u64 = strategies.iter().map(|s| s.state.metrics.losses).sum();
    let win_rate = if total_wins + total_losses > 0 {
        total_wins as f64 / (total_wins + total_losses) as f64
    } else {
        0.0
    };

    // Session summary
    log_session_summary(
        duration.as_secs(),
        total_pnl,
        max_drawdown,
        total_trades,
        win_rate,
        0, // halts
        drift_events,
    );

    // Final system log
    log(
        Level::Info,
        Domain::System,
        "shutdown",
        obj(&[
            ("duration_ms", v_num(duration.as_millis() as f64)),
            ("total_bars", v_num(n_bars as f64)),
            ("bars_per_sec", v_num(n_bars as f64 / duration.as_secs_f64())),
            ("total_pnl", v_num(total_pnl)),
            ("max_drawdown", v_num(max_drawdown)),
        ]),
    );

    // Print human-readable summary
    println!("\n=== Backtest Complete ===");
    println!("Bars: {}", n_bars);
    println!("Duration: {:.2?}", duration);
    println!("Throughput: {:.0} bars/sec", n_bars as f64 / duration.as_secs_f64());
    println!("Total trades: {}", total_trades);
    println!("Win rate: {:.1}%", win_rate * 100.0);
    println!("Total PnL: {:.2}", total_pnl);
    println!("Max drawdown: {:.2}%", max_drawdown * 100.0);
    println!("Drift events: {}", drift_events);
    println!("\nLogs written to out/runs/");
}
