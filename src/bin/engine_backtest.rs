//! Backtest using the event-driven engine for transparency.

use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

use arbitragefx::engine::{
    bus::EventBus,
    events::*,
    reducer::{reduce, ReducerConfig},
    state::EngineState,
};

fn parse_csv_line(line: &str) -> Result<(u64, f64, f64, f64, f64, f64, f64, f64, f64, f64)> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 10 {
        return Err(anyhow!("need 10 columns"));
    }
    Ok((
        parts[0].trim().parse()?,
        parts[1].trim().parse()?, // o
        parts[2].trim().parse()?, // h
        parts[3].trim().parse()?, // l
        parts[4].trim().parse()?, // c
        parts[5].trim().parse()?, // v
        parts[6].trim().parse()?, // funding
        parts[7].trim().parse()?, // borrow
        parts[8].trim().parse()?, // liq
        parts[9].trim().parse()?, // depeg
    ))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: engine_backtest <csv_file>");
        std::process::exit(1);
    }

    let file = File::open(&args[1])?;
    let reader = BufReader::new(file);
    let symbol = std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());

    let mut state = EngineState::new();
    let mut bus = EventBus::new();

    // Configure for mean-reversion strategy (non-grasping)
    //
    // Philosophy alignment:
    // - entry_threshold: require clear exhaustion, don't chase
    // - position_size: small, accept that most trades are learning
    // - take_profit/stop_loss: accept outcomes without attachment
    let cfg = ReducerConfig {
        entry_threshold: 0.25,     // Mean-reversion score threshold
        exit_threshold: 0.15,      // Exit when signal weakens
        position_size: 0.01,       // 1% of equity per trade
        max_position_pct: 0.10,    // Max 10% exposure
        take_profit_pct: 0.004,    // 0.4% take profit
        stop_loss_pct: 0.003,      // 0.3% stop loss
        cooldown_ms: 0,            // No cooldown in backtest (faster iteration)
        data_stale_ms: u64::MAX,   // Don't halt on stale data in backtest
        ..Default::default()
    };

    let mut trades = 0;
    let mut row_count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("ts") || line.is_empty() {
            continue; // skip header
        }

        let (ts, o, h, l, c, v, funding, _borrow, liq, _depeg) = parse_csv_line(&line)?;
        row_count += 1;

        // Push candle event
        bus.push(Event::Market(MarketEvent::Candle {
            ts: ts * 1000,
            symbol: symbol.clone(),
            o, h, l, c, v,
        }));

        // Push funding if significant
        if funding.abs() > 0.00001 {
            bus.push(Event::Market(MarketEvent::Funding {
                ts: ts * 1000,
                symbol: symbol.clone(),
                rate: funding,
                next_ts: ts * 1000 + 8 * 3600 * 1000,
            }));
        }

        // Push liquidation events if significant
        if liq > 1.0 {
            bus.push(Event::Market(MarketEvent::Liquidation {
                ts: ts * 1000,
                symbol: symbol.clone(),
                side: TradeSide::Sell,
                qty: liq * 0.01,
                price: c,
            }));
        }

        // Timer for housekeeping
        bus.push(Event::Sys(SysEvent::Timer {
            ts: ts * 1000,
            name: "tick".to_string(),
        }));

        // Process all events
        while let Some(event) = bus.pop() {
            let output = reduce(&mut state, event, &cfg);

            for cmd in output.commands {
                match &cmd {
                    Command::PlaceOrder { symbol, side, qty, .. } => {
                        trades += 1;
                        let side_str = if matches!(side, TradeSide::Buy) { "BUY" } else { "SELL" };
                        eprintln!("[{:>4}] {} {} {:.6} @ {:.2}", row_count, side_str, symbol, qty, c);

                        // Simulate immediate fill
                        let fill_price = if matches!(side, TradeSide::Buy) {
                            c * 1.001 // 0.1% slippage
                        } else {
                            c * 0.999
                        };
                        let fee = qty * fill_price * 0.001; // 0.1% fee

                        bus.push(Event::Exec(ExecEvent::Fill {
                            ts: ts * 1000,
                            symbol: symbol.clone(),
                            client_id: format!("bt-{}", trades),
                            order_id: format!("ex-{}", trades),
                            fill_id: format!("f-{}", trades),
                            price: fill_price,
                            qty: *qty,
                            fee,
                            side: *side,
                        }));
                    }
                    Command::Log { level, msg } => {
                        if matches!(level, LogLevel::Info | LogLevel::Warn | LogLevel::Error) {
                            eprintln!("[LOG] {}", msg);
                        }
                    }
                    Command::Halt { reason } => {
                        eprintln!("[HALT] {:?}", reason);
                    }
                    _ => {}
                }
            }
        }

        // Print periodic status with mean-reversion indicators
        if row_count % 10 == 0 {
            if let Some(sym) = state.symbols.get(&symbol) {
                eprintln!(
                    "[{:>4}] price={:.2} rsi={:.1} mr_score={:.3} eq={:.2} pos={:.6}",
                    row_count,
                    sym.last_price,
                    sym.rsi(),
                    sym.mean_reversion_score(),
                    state.portfolio.equity,
                    state.portfolio.positions.get(&symbol).map(|p| p.qty).unwrap_or(0.0),
                );
            }
        }
    }

    // Final summary
    println!("\n=== BACKTEST RESULTS ===");
    println!("Rows processed: {}", row_count);
    println!("Total trades: {}", trades);
    println!("Final equity: {:.2}", state.portfolio.equity);
    println!("Realized PnL: {:.4}", state.portfolio.realized_pnl);
    println!("Max drawdown: {:.2}%", state.portfolio.drawdown_pct() * 100.0);
    println!("State hash: {}", state.hash());

    if let Some(sym) = state.symbols.get(&symbol) {
        println!("\nFinal market state:");
        println!("  Price: {:.2}", sym.last_price);
        println!("  Z-momentum: {:.3}", sym.z_momentum());
        println!("  Volatility: {:.4}", sym.volatility);
        println!("  Candles: {}", sym.candle_count);
    }

    if let Some(pos) = state.portfolio.positions.get(&symbol) {
        if pos.qty != 0.0 {
            println!("\nOpen position:");
            println!("  Qty: {:.6}", pos.qty);
            println!("  Entry: {:.2}", pos.entry_price);
        }
    }

    Ok(())
}
