//! Strategy sweep - tests all composable strategies on historical data.
//!
//! Usage: cargo run --release --bin strategy_sweep -- [data.csv]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::strategies::{
    AdaptiveStrategy, EventDrivenStrategy, FundingCarryStrategy, MeanReversionStrategy,
    MomentumStrategy, MultiFactorStrategy, StrategyConfig, VolatilityBreakoutStrategy,
};
use arbitragefx::strategy::{Action, Candle, IndicatorSnapshot, MarketAux, MarketView, PortfolioState, Strategy, StrategyState, MetricsState};

fn load_data(path: &str) -> Vec<CsvRow> {
    let file = File::open(path).expect("Failed to open data file");
    let reader = BufReader::new(file);
    reader
        .lines()
        .skip(1)
        .filter_map(|l| l.ok())
        .filter_map(|l| parse_csv_line(&l).ok())
        .collect()
}

/// Running statistics for indicators
struct RunningStats {
    ema_fast: f64,
    ema_slow: f64,
    vol_sum: f64,
    vol_count: u64,
    price_sum: f64,
    price_sq_sum: f64,
    momentum_prev: f64,
    volume_sum: f64,
}

impl RunningStats {
    fn new() -> Self {
        Self {
            ema_fast: 0.0,
            ema_slow: 0.0,
            vol_sum: 0.0,
            vol_count: 0,
            price_sum: 0.0,
            price_sq_sum: 0.0,
            momentum_prev: 0.0,
            volume_sum: 0.0,
        }
    }

    fn update(&mut self, row: &CsvRow) -> IndicatorSnapshot {
        let alpha_fast = 0.1;
        let alpha_slow = 0.02;

        // EMA updates
        if self.ema_fast == 0.0 {
            self.ema_fast = row.c;
            self.ema_slow = row.c;
        } else {
            self.ema_fast = self.ema_fast * (1.0 - alpha_fast) + row.c * alpha_fast;
            self.ema_slow = self.ema_slow * (1.0 - alpha_slow) + row.c * alpha_slow;
        }

        // Volatility (using H-L range)
        let vol = (row.h - row.l) / row.c;
        self.vol_sum += vol;
        self.vol_count += 1;
        let vol_mean = self.vol_sum / self.vol_count as f64;

        // Price stats for z-score
        self.price_sum += row.c;
        self.price_sq_sum += row.c * row.c;
        let n = self.vol_count as f64;
        let price_mean = self.price_sum / n;
        let price_var = if n > 1.0 {
            (self.price_sq_sum - self.price_sum * self.price_sum / n) / (n - 1.0)
        } else {
            1.0
        };
        let price_std = price_var.sqrt().max(0.001);

        // Momentum
        let momentum = row.c - self.ema_fast;
        let mom_change = momentum - self.momentum_prev;
        self.momentum_prev = momentum;

        // Volume spike
        self.volume_sum += row.v;
        let vol_mean_v = self.volume_sum / n;
        let volume_spike = if vol_mean_v > 0.0 { row.v / vol_mean_v } else { 1.0 };

        // Z-scores (simplified)
        let z_momentum = momentum / price_std;
        let z_stretch = (row.c - self.ema_slow) / price_std;
        let z_vol = (vol - vol_mean) / vol_mean.max(0.001);
        let z_volume_spike = (volume_spike - 1.0).max(-2.0).min(3.0);

        IndicatorSnapshot {
            ema_fast: self.ema_fast,
            ema_slow: self.ema_slow,
            vwap: price_mean,
            vol,
            vol_mean,
            momentum,
            volume_spike,
            stretch: row.c - self.ema_slow,
            z_momentum,
            z_vol,
            z_volume_spike,
            z_stretch,
        }
    }
}

struct StrategyResult {
    name: &'static str,
    pnl: f64,
    trades: u64,
    wins: u64,
    max_dd: f64,
    runtime_ms: u64,
}

fn run_strategy<S: Strategy>(mut strat: S, rows: &[CsvRow]) -> StrategyResult {
    let start = Instant::now();

    let mut state = StrategyState {
        portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
        metrics: MetricsState::default(),
        last_trade_ts: 0,
        last_loss_ts: 0,
        trading_halted: false,
        trades_today: 0,
        trade_day: 0,
        order_seq: 0,
    };
    state.metrics.equity_peak = 1000.0;

    let mut stats = RunningStats::new();
    let mut trades = 0u64;
    let mut wins = 0u64;

    for row in rows {
        let indicators = stats.update(row);
        let aux = MarketAux {
            funding_rate: row.funding,
            borrow_rate: row.borrow,
            liquidation_score: row.liq,
            stable_depeg: row.depeg,
            fetch_ts: row.ts,
            has_funding: row.funding != 0.0,
            has_borrow: row.borrow != 0.0,
            has_liquidations: row.liq != 0.0,
            has_depeg: row.depeg != 0.0,
        };

        let market = MarketView {
            symbol: "BTCUSDT",
            last: Candle { ts: row.ts, o: row.o, h: row.h, l: row.l, c: row.c, v: row.v },
            indicators,
            aux,
        };

        let action = strat.update(market, &mut state);

        // Execute action
        let fill = match action {
            Action::Hold => None,
            Action::Close => {
                let qty = -state.portfolio.position;
                if qty.abs() > 1e-9 { Some((qty, row.c)) } else { None }
            }
            Action::Buy { qty } => Some((qty.min(0.001), row.c)),
            Action::Sell { qty } => Some((-qty.min(0.001), row.c)),
        };

        if let Some((qty, price)) = fill {
            let fee = price * qty.abs() * 0.001;
            let prev_pos = state.portfolio.position;

            // Calculate realized PnL
            let realized = if prev_pos != 0.0 && prev_pos.signum() != qty.signum() {
                let close_qty = prev_pos.abs().min(qty.abs());
                let dir = if prev_pos > 0.0 { 1.0 } else { -1.0 };
                (price - state.portfolio.entry_price) * close_qty * dir
            } else {
                0.0
            };

            // Update portfolio
            state.portfolio.cash -= price * qty + fee;
            state.portfolio.position += qty;
            if prev_pos == 0.0 || prev_pos.signum() != state.portfolio.position.signum() {
                state.portfolio.entry_price = price;
            }
            state.portfolio.equity = state.portfolio.cash + state.portfolio.position * price;

            state.metrics.pnl += realized - fee;
            if realized > fee {
                wins += 1;
            }
            trades += 1;
            state.last_trade_ts = row.ts;

            // Update drawdown
            if state.portfolio.equity > state.metrics.equity_peak {
                state.metrics.equity_peak = state.portfolio.equity;
            }
            let dd = (state.portfolio.equity - state.metrics.equity_peak) / state.metrics.equity_peak;
            if dd < state.metrics.max_drawdown {
                state.metrics.max_drawdown = dd;
            }
        }
    }

    // Close any remaining position
    if let Some(last) = rows.last() {
        if state.portfolio.position.abs() > 1e-9 {
            let qty = -state.portfolio.position;
            let fee = last.c * qty.abs() * 0.001;
            let realized = if state.portfolio.position > 0.0 {
                (last.c - state.portfolio.entry_price) * state.portfolio.position.abs()
            } else {
                (state.portfolio.entry_price - last.c) * state.portfolio.position.abs()
            };
            state.metrics.pnl += realized - fee;
            if realized > fee { wins += 1; }
            trades += 1;
        }
    }

    StrategyResult {
        name: strat.id(),
        pnl: state.metrics.pnl,
        trades,
        wins,
        max_dd: state.metrics.max_drawdown,
        runtime_ms: start.elapsed().as_millis() as u64,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data_path = args.get(1).map(|s| s.as_str()).unwrap_or("data/btc_1h_180d.csv");

    println!("Loading data from {}...", data_path);
    let rows = load_data(data_path);
    println!("Loaded {} bars", rows.len());

    if rows.len() < 100 {
        eprintln!("Not enough data (need at least 100 bars)");
        return;
    }

    let first_price = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last_price = rows.last().map(|r| r.c).unwrap_or(0.0);
    println!("Price range: {:.2} → {:.2} (buy-hold: {:.2})\n", first_price, last_price, last_price - first_price);

    println!("{:<25} {:>10} {:>8} {:>8} {:>10} {:>8}",
             "Strategy", "PnL", "Trades", "Wins", "MaxDD", "ms");
    println!("{}", "-".repeat(75));

    // Run each strategy
    let strategies: Vec<Box<dyn Fn() -> Box<dyn Strategy>>> = vec![
        Box::new(|| Box::new(MomentumStrategy::default())),
        Box::new(|| Box::new(MomentumStrategy {
            require_trend_align: false,
            ..Default::default()
        })),
        Box::new(|| Box::new(MeanReversionStrategy::default())),
        Box::new(|| Box::new(MeanReversionStrategy {
            require_trend_align: false,
            ..Default::default()
        })),
        Box::new(|| Box::new(FundingCarryStrategy::default())),
        Box::new(|| Box::new(VolatilityBreakoutStrategy::default())),
        Box::new(|| Box::new(EventDrivenStrategy::default())),
        Box::new(|| Box::new(MultiFactorStrategy::default())),
        Box::new(|| Box::new(MultiFactorStrategy::trend_following())),
        Box::new(|| Box::new(MultiFactorStrategy::mean_reversion_focused())),
        Box::new(|| Box::new(MultiFactorStrategy::carry_focused())),
        Box::new(|| Box::new(AdaptiveStrategy::default())),
        // Aggressive variants
        Box::new(|| Box::new(MomentumStrategy {
            config: StrategyConfig { entry_threshold: 0.15, stop_loss: 0.03, take_profit: 0.04, ..Default::default() },
            momentum_threshold: 0.8,
            require_trend_align: true,
        })),
        // Conservative variants
        Box::new(|| Box::new(MomentumStrategy {
            config: StrategyConfig { entry_threshold: 0.5, stop_loss: 0.015, take_profit: 0.02, ..Default::default() },
            momentum_threshold: 1.5,
            require_trend_align: true,
        })),
    ];

    let names = [
        "momentum (trend-aligned)",
        "momentum (pure)",
        "mean_reversion (aligned)",
        "mean_reversion (pure)",
        "funding_carry",
        "vol_breakout",
        "event_driven",
        "multi_factor (default)",
        "multi_factor (trend)",
        "multi_factor (revert)",
        "multi_factor (carry)",
        "adaptive",
        "momentum (aggressive)",
        "momentum (conservative)",
    ];

    let mut results = Vec::new();

    for (i, make_strat) in strategies.iter().enumerate() {
        let strat = make_strat();
        let result = run_strategy_boxed(strat, &rows);
        let win_pct = if result.trades > 0 { result.wins as f64 / result.trades as f64 * 100.0 } else { 0.0 };
        println!("{:<25} {:>10.2} {:>8} {:>7.1}% {:>9.2}% {:>8}",
                 names[i], result.pnl, result.trades, win_pct, result.max_dd * 100.0, result.runtime_ms);
        results.push((names[i], result));
    }

    // Summary
    println!("\n=== Summary ===");
    let profitable: Vec<_> = results.iter().filter(|(_, r)| r.pnl > 0.0).collect();
    println!("Profitable strategies: {}/{}", profitable.len(), results.len());

    if !profitable.is_empty() {
        println!("\nProfitable:");
        for (name, r) in &profitable {
            println!("  {} → PnL: {:.2}, Trades: {}", name, r.pnl, r.trades);
        }
    }

    let best = results.iter().max_by(|a, b| a.1.pnl.partial_cmp(&b.1.pnl).unwrap());
    if let Some((name, r)) = best {
        println!("\nBest: {} (PnL: {:.2})", name, r.pnl);
    }
}

fn run_strategy_boxed(mut strat: Box<dyn Strategy>, rows: &[CsvRow]) -> StrategyResult {
    let start = Instant::now();

    let mut state = StrategyState {
        portfolio: PortfolioState { cash: 1000.0, position: 0.0, entry_price: 0.0, equity: 1000.0 },
        metrics: MetricsState::default(),
        last_trade_ts: 0,
        last_loss_ts: 0,
        trading_halted: false,
        trades_today: 0,
        trade_day: 0,
        order_seq: 0,
    };
    state.metrics.equity_peak = 1000.0;

    let mut stats = RunningStats::new();
    let mut trades = 0u64;
    let mut wins = 0u64;

    for row in rows {
        let indicators = stats.update(row);
        let aux = MarketAux {
            funding_rate: row.funding,
            borrow_rate: row.borrow,
            liquidation_score: row.liq,
            stable_depeg: row.depeg,
            fetch_ts: row.ts,
            has_funding: row.funding != 0.0,
            has_borrow: row.borrow != 0.0,
            has_liquidations: row.liq != 0.0,
            has_depeg: row.depeg != 0.0,
        };

        let market = MarketView {
            symbol: "BTCUSDT",
            last: Candle { ts: row.ts, o: row.o, h: row.h, l: row.l, c: row.c, v: row.v },
            indicators,
            aux,
        };

        let action = strat.update(market, &mut state);

        let fill = match action {
            Action::Hold => None,
            Action::Close => {
                let qty = -state.portfolio.position;
                if qty.abs() > 1e-9 { Some((qty, row.c)) } else { None }
            }
            Action::Buy { qty } => Some((qty.min(0.001), row.c)),
            Action::Sell { qty } => Some((-qty.min(0.001), row.c)),
        };

        if let Some((qty, price)) = fill {
            let fee = price * qty.abs() * 0.001;
            let prev_pos = state.portfolio.position;

            let realized = if prev_pos != 0.0 && prev_pos.signum() != qty.signum() {
                let close_qty = prev_pos.abs().min(qty.abs());
                let dir = if prev_pos > 0.0 { 1.0 } else { -1.0 };
                (price - state.portfolio.entry_price) * close_qty * dir
            } else {
                0.0
            };

            state.portfolio.cash -= price * qty + fee;
            state.portfolio.position += qty;
            if prev_pos == 0.0 || prev_pos.signum() != state.portfolio.position.signum() {
                state.portfolio.entry_price = price;
            }
            state.portfolio.equity = state.portfolio.cash + state.portfolio.position * price;

            state.metrics.pnl += realized - fee;
            if realized > fee { wins += 1; }
            trades += 1;
            state.last_trade_ts = row.ts;

            if state.portfolio.equity > state.metrics.equity_peak {
                state.metrics.equity_peak = state.portfolio.equity;
            }
            let dd = (state.portfolio.equity - state.metrics.equity_peak) / state.metrics.equity_peak;
            if dd < state.metrics.max_drawdown {
                state.metrics.max_drawdown = dd;
            }
        }
    }

    if let Some(last) = rows.last() {
        if state.portfolio.position.abs() > 1e-9 {
            let qty = -state.portfolio.position;
            let fee = last.c * qty.abs() * 0.001;
            let realized = if state.portfolio.position > 0.0 {
                (last.c - state.portfolio.entry_price) * state.portfolio.position.abs()
            } else {
                (state.portfolio.entry_price - last.c) * state.portfolio.position.abs()
            };
            state.metrics.pnl += realized - fee;
            if realized > fee { wins += 1; }
            trades += 1;
        }
    }

    StrategyResult {
        name: strat.id(),
        pnl: state.metrics.pnl,
        trades,
        wins,
        max_dd: state.metrics.max_drawdown,
        runtime_ms: start.elapsed().as_millis() as u64,
    }
}
