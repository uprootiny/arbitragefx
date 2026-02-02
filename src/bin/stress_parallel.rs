//! Parallel stress test - tests thread safety and memory under concurrent load.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use arbitragefx::backtest::CsvRow;
use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, Fill, MarketState, StrategyInstance};
use arbitragefx::strategy::{Action, MarketAux};

fn generate_data(n_bars: usize, seed: u64) -> Vec<CsvRow> {
    let mut rows = Vec::with_capacity(n_bars);
    let mut price = 50000.0 + (seed as f64 * 100.0);
    let mut ts = 1700000000u64 + seed * 1000;

    for i in 0..n_bars {
        let noise = ((i as f64 * 0.1 + seed as f64).sin() * 100.0);
        price = (price + noise).max(1.0);

        rows.push(CsvRow {
            ts,
            o: price * 0.999,
            h: price * 1.01,
            l: price * 0.99,
            c: price,
            v: 1000.0,
            funding: 0.0001,
            borrow: 0.00005,
            liq: 0.5,
            depeg: 0.0,
            oi: 1_000_000.0,
        });
        ts += 300;
    }
    rows
}

fn run_backtest(rows: &[CsvRow], cfg: &Config) -> u64 {
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();
    let mut trades = 0u64;

    for row in rows {
        let candle = arbitragefx::exchange::Candle {
            ts: row.ts, o: row.o, h: row.h, l: row.l, c: row.c, v: row.v,
        };
        market.on_candle(candle);
        market.update_aux(&cfg.symbol, MarketAux {
            funding_rate: row.funding,
            borrow_rate: row.borrow,
            liquidation_score: row.liq,
            stable_depeg: row.depeg,
            fetch_ts: row.ts,
            has_funding: true,
            has_borrow: true,
            has_liquidations: true,
            has_depeg: false,
        });

        for inst in strategies.iter_mut() {
            let view = market.view(&cfg.symbol);
            let action = inst.strategy.update(view, &mut inst.state);
            let guarded = risk.apply_with_price(&inst.state, action, row.ts, row.c);

            if let Some((qty, price)) = match guarded {
                Action::Hold => None,
                Action::Close => {
                    let q = -inst.state.portfolio.position;
                    if q.abs() > 1e-9 { Some((q, row.c)) } else { None }
                }
                Action::Buy { qty } => Some((qty, row.c)),
                Action::Sell { qty } => Some((-qty.abs(), row.c)),
            } {
                let fee = price * qty.abs() * 0.001;
                inst.state.portfolio.apply_fill(Fill { price, qty, fee, ts: row.ts });
                trades += 1;
            }
            metrics.update_with_price(&mut inst.state, row.c);
        }
    }
    trades
}

fn get_memory_mb() -> f64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<usize>().ok())
        .map(|pages| pages as f64 * 4096.0 / 1_000_000.0)
        .unwrap_or(0.0)
}

fn main() {
    println!("=== PARALLEL STRESS TEST ===\n");

    let n_threads = num_cpus::get().min(8);
    let bars_per_thread = 10_000;
    let iterations = 50;

    println!("Threads: {}", n_threads);
    println!("Bars per thread: {}", bars_per_thread);
    println!("Iterations: {}", iterations);
    println!();

    let cfg = Config::from_env();
    let total_trades = Arc::new(AtomicU64::new(0));
    let total_bars = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let initial_mem = get_memory_mb();

    println!("{:>10} {:>12} {:>12} {:>12} {:>10}",
             "Iteration", "Time", "Trades", "Memory MB", "Bars/sec");
    println!("{}", "-".repeat(60));

    for iter in 0..iterations {
        let iter_start = Instant::now();
        let mut handles = vec![];

        for t in 0..n_threads {
            let cfg = cfg.clone();
            let trades_counter = Arc::clone(&total_trades);
            let bars_counter = Arc::clone(&total_bars);
            let seed = (iter * n_threads + t) as u64;

            handles.push(thread::spawn(move || {
                let data = generate_data(bars_per_thread, seed);
                let trades = run_backtest(&data, &cfg);
                trades_counter.fetch_add(trades, Ordering::Relaxed);
                bars_counter.fetch_add(bars_per_thread as u64, Ordering::Relaxed);
            }));
        }

        for h in handles {
            h.join().expect("Thread panicked!");
        }

        let iter_time = iter_start.elapsed();
        let curr_mem = get_memory_mb();
        let bars_this_iter = (n_threads * bars_per_thread) as f64;
        let bars_per_sec = bars_this_iter / iter_time.as_secs_f64();

        if iter % 10 == 9 || iter == 0 {
            println!("{:>10} {:>12} {:>12} {:>12.1} {:>10.0}",
                     iter + 1,
                     format!("{:.2?}", iter_time),
                     total_trades.load(Ordering::Relaxed),
                     curr_mem,
                     bars_per_sec);
        }

        // Memory check
        if curr_mem > 500.0 {
            println!("\n⚠️  Memory > 500MB at iteration {}, investigating...", iter + 1);
            // Force GC by dropping any cached data
            thread::sleep(Duration::from_millis(100));
            let after_gc = get_memory_mb();
            println!("After brief pause: {:.1} MB", after_gc);
            if after_gc > 800.0 {
                println!("❌ Memory leak suspected, stopping");
                break;
            }
        }
    }

    let total_time = start.elapsed();
    let final_mem = get_memory_mb();

    println!("\n=== Summary ===");
    println!("Total time: {:.2?}", total_time);
    println!("Total bars processed: {}", total_bars.load(Ordering::Relaxed));
    println!("Total trades executed: {}", total_trades.load(Ordering::Relaxed));
    println!("Overall throughput: {:.0} bars/sec",
             total_bars.load(Ordering::Relaxed) as f64 / total_time.as_secs_f64());
    println!("Memory: {:.1} MB initial → {:.1} MB final (Δ {:.1} MB)",
             initial_mem, final_mem, final_mem - initial_mem);

    if final_mem - initial_mem < 50.0 {
        println!("\n✓ No significant memory leak detected");
    } else {
        println!("\n⚠️  Memory grew by {:.1} MB - investigate", final_mem - initial_mem);
    }

    println!("\n✓ Parallel stress test complete");
}
