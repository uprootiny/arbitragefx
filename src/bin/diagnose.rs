//! Diagnostic tool to understand strategy behavior.

use std::fs::File;
use std::io::{BufRead, BufReader};

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, Fill, MarketState, StrategyInstance};
use arbitragefx::strategy::{Action, MarketAux};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/btc_binance.csv".to_string());

    let file = File::open(&path).expect("Failed to open file");
    let mut rows: Vec<CsvRow> = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') || line.to_lowercase().starts_with("ts,")
        {
            continue;
        }
        if let Ok(r) = parse_csv_line(&line) {
            rows.push(r);
        }
    }

    println!("Loaded {} bars", rows.len());
    println!();

    let cfg = Config::from_env();
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());

    let mut action_counts: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut guard_reasons: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let mut positions: Vec<(u64, f64, f64)> = Vec::new(); // (ts, position, price)

    for (bar_idx, row) in rows.iter().enumerate() {
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

        // Only look at first strategy for diagnostics
        let inst = &mut strategies[0];
        let view = market.view(&cfg.symbol);
        let raw_action = inst.strategy.update(view, &mut inst.state);
        let guarded = risk.apply_with_price(&inst.state, raw_action, row.ts, row.c);

        // Count actions
        let action_name = match raw_action {
            Action::Hold => "Hold",
            Action::Buy { .. } => "Buy",
            Action::Sell { .. } => "Sell",
            Action::Close => "Close",
        };
        *action_counts.entry(action_name.to_string()).or_insert(0) += 1;

        // Count guard blocks
        if !matches!(raw_action, Action::Hold) && matches!(guarded, Action::Hold) {
            *guard_reasons.entry("blocked".to_string()).or_insert(0) += 1;
        }

        // Apply fills
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
            Action::Buy { qty } => Some((qty, row.c)),
            Action::Sell { qty } => Some((-qty.abs(), row.c)),
        };

        if let Some((qty, price)) = fill {
            let fee = price * qty.abs() * 0.001;
            let realized = inst.state.portfolio.apply_fill(Fill {
                price,
                qty,
                fee,
                ts: row.ts,
            });

            // Log trades
            if bar_idx < 50 || realized.abs() > 0.0 {
                println!(
                    "Bar {}: {} @ {:.2} qty={:.4} realized={:.4} pos={:.4}",
                    bar_idx, action_name, price, qty, realized, inst.state.portfolio.position
                );
            }

            inst.state.metrics.record_trade(realized);
        }

        // Mark to market equity and track drawdown
        inst.state.portfolio.equity =
            inst.state.portfolio.cash + inst.state.portfolio.position * row.c;
        if inst.state.portfolio.equity > inst.state.metrics.equity_peak {
            inst.state.metrics.equity_peak = inst.state.portfolio.equity;
        }
        if inst.state.metrics.equity_peak > 0.0 {
            let dd = (inst.state.portfolio.equity - inst.state.metrics.equity_peak)
                / inst.state.metrics.equity_peak;
            if dd < inst.state.metrics.max_drawdown {
                inst.state.metrics.max_drawdown = dd;
            }
        }

        // Track position over time
        if bar_idx % 100 == 0 {
            positions.push((row.ts, inst.state.portfolio.position, row.c));
        }
    }

    println!();
    println!("=== Action Distribution ===");
    for (action, count) in &action_counts {
        println!(
            "{}: {} ({:.1}%)",
            action,
            count,
            100.0 * *count as f64 / rows.len() as f64
        );
    }

    println!();
    println!("=== Guard Stats ===");
    for (reason, count) in &guard_reasons {
        println!("{}: {}", reason, count);
    }

    println!();
    println!("=== Position Samples ===");
    for (ts, pos, price) in &positions {
        println!("ts={} pos={:.4} price={:.2}", ts, pos, price);
    }

    let inst = &strategies[0];
    println!();
    println!("=== Final State ===");
    println!("PnL: {:.4}", inst.state.metrics.pnl);
    println!("Wins: {}", inst.state.metrics.wins);
    println!("Losses: {}", inst.state.metrics.losses);
    println!(
        "Win Rate: {:.1}%",
        if inst.state.metrics.wins + inst.state.metrics.losses > 0 {
            100.0 * inst.state.metrics.wins as f64
                / (inst.state.metrics.wins + inst.state.metrics.losses) as f64
        } else {
            0.0
        }
    );
    println!("Expectancy: {:.4}", inst.state.metrics.expectancy());
    println!("Position: {:.6}", inst.state.portfolio.position);
    println!("Equity: {:.2}", inst.state.portfolio.equity);
    println!("Max DD: {:.2}%", inst.state.metrics.max_drawdown * 100.0);

    // Check indicator values
    println!();
    println!("=== Last Market View ===");
    let view = market.view(&cfg.symbol);
    println!("Price: {:.2}", view.last.c);
    println!("EMA fast: {:.2}", view.indicators.ema_fast);
    println!("EMA slow: {:.2}", view.indicators.ema_slow);
    println!("Momentum: {:.4}", view.indicators.momentum);
    println!("Z-momentum: {:.4}", view.indicators.z_momentum);
    println!("Z-vol: {:.4}", view.indicators.z_vol);
    println!(
        "Vol/Vol_mean: {:.4}",
        view.indicators.vol / view.indicators.vol_mean.max(1e-9)
    );
}
