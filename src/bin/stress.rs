//! Stress test for backtest system.
//!
//! Runs backtests with progressively larger datasets to study:
//! - Memory usage
//! - Processing time scaling
//! - Throttling behavior
//!
//! Usage: cargo run --release --bin stress

use std::time::Instant;

use arbitragefx::backtest::CsvRow;
use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, Fill, MarketState, StrategyInstance};
use arbitragefx::strategy::{Action, MarketAux};

fn generate_synthetic_data(n_bars: usize, base_price: f64) -> Vec<CsvRow> {
    let mut rows = Vec::with_capacity(n_bars);
    let mut price = base_price;
    let mut ts = 1700000000u64;

    for i in 0..n_bars {
        // Random walk with mean reversion
        let drift = (base_price - price) * 0.001; // Mean reversion
        let noise = ((i as f64 * 0.1).sin() * 100.0) + ((i as f64 * 0.3).cos() * 50.0);
        price = (price + drift + noise).max(1.0);

        let volatility = 0.02;
        let h = price * (1.0 + volatility * 0.5);
        let l = price * (1.0 - volatility * 0.5);
        let o = price * (1.0 + volatility * 0.1 * (i as f64).sin());
        let c = price;

        rows.push(CsvRow {
            ts,
            o,
            h,
            l,
            c,
            v: 1000.0 + (i as f64 % 500.0),
            funding: 0.0001 * (i as f64 % 10.0 - 5.0) / 5.0,
            borrow: 0.00005,
            liq: 0.5,
            depeg: 0.0,
            oi: 1_000_000.0,
        });

        ts += 300; // 5 min bars
    }

    rows
}

fn run_backtest(rows: &[CsvRow], cfg: &Config) -> (f64, f64, u64, u64) {
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();

    let mut total_trades = 0u64;

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
                inst.state.metrics.record_trade(realized);
                total_trades += 1;
            }

            metrics.update_with_price(&mut inst.state, row.c);
        }
    }

    let total_pnl: f64 = strategies.iter().map(|s| s.state.metrics.pnl).sum();
    let max_dd: f64 = strategies
        .iter()
        .map(|s| s.state.metrics.max_drawdown)
        .fold(0.0, f64::min);
    let n_strategies = strategies.len() as u64;

    (total_pnl, max_dd, total_trades, n_strategies)
}

fn get_memory_usage() -> usize {
    // Try to read from /proc/self/statm on Linux
    if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
        if let Some(rss) = content.split_whitespace().nth(1) {
            if let Ok(pages) = rss.parse::<usize>() {
                return pages * 4096; // Page size typically 4KB
            }
        }
    }
    0
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.2} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

fn main() {
    println!("=== STRESS TEST: Backtest System ===\n");

    let cfg = Config::from_env();
    let base_price = 50000.0;

    // Progressive load sizes
    let sizes = [
        100, 500, 1_000, 5_000, 10_000, 50_000, 100_000, 500_000, 1_000_000,
    ];

    println!(
        "{:>10} {:>12} {:>12} {:>10} {:>10} {:>12} {:>10}",
        "Bars", "Gen Time", "Run Time", "Trades", "PnL", "Memory", "Bars/sec"
    );
    println!("{}", "-".repeat(88));

    let mut prev_mem = get_memory_usage();

    for &n_bars in &sizes {
        // Generate data
        let gen_start = Instant::now();
        let rows = generate_synthetic_data(n_bars, base_price);
        let gen_time = gen_start.elapsed();

        // Run backtest
        let run_start = Instant::now();
        let (pnl, _max_dd, trades, _n_strat) = run_backtest(&rows, &cfg);
        let run_time = run_start.elapsed();

        // Memory check
        let curr_mem = get_memory_usage();
        let mem_delta = curr_mem.saturating_sub(prev_mem);

        // Calculate throughput
        let bars_per_sec = if run_time.as_secs_f64() > 0.0 {
            n_bars as f64 / run_time.as_secs_f64()
        } else {
            0.0
        };

        println!(
            "{:>10} {:>12} {:>12} {:>10} {:>10.2} {:>12} {:>10.0}",
            n_bars,
            format!("{:.2?}", gen_time),
            format!("{:.2?}", run_time),
            trades,
            pnl,
            format_bytes(mem_delta),
            bars_per_sec
        );

        prev_mem = curr_mem;

        // Check for OOM risk
        if curr_mem > 1_000_000_000 {
            println!("\n⚠️  Memory usage > 1GB, stopping to prevent OOM");
            break;
        }

        // Drop data explicitly to allow GC
        drop(rows);
    }

    println!("\n=== Summary ===");
    println!("Final memory: {}", format_bytes(get_memory_usage()));

    // Test rapid iteration (throttling behavior)
    println!("\n=== Rapid Iteration Test (1000 bars × 100 iterations) ===");
    let rows = generate_synthetic_data(1000, base_price);
    let start = Instant::now();
    let mut total_trades = 0u64;

    for i in 0..100 {
        let (_, _, trades, _) = run_backtest(&rows, &cfg);
        total_trades += trades;

        if i % 20 == 19 {
            let elapsed = start.elapsed();
            let rate = (i + 1) as f64 / elapsed.as_secs_f64();
            println!(
                "  Iteration {}: {:.1} runs/sec, {} total trades",
                i + 1,
                rate,
                total_trades
            );
        }
    }

    let total_time = start.elapsed();
    println!("\nCompleted 100 iterations in {:.2?}", total_time);
    println!("Average: {:.2?} per iteration", total_time / 100);
    println!(
        "Throughput: {:.0} bars/sec overall",
        100_000.0 / total_time.as_secs_f64()
    );

    println!("\n✓ Stress test complete - no OOM or crashes");
}
