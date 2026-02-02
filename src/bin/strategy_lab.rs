//! Strategy Lab - comprehensive testing of all strategy building blocks.
//!
//! Tests combinations of signals, filters, and sizing rules.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::filters::{
    all_filters, drawdown_filter, momentum_filter, position_limit_filter,
    trend_alignment_filter, volatility_filter, FilterResult,
};
use arbitragefx::indicators::{
    Atr, BollingerBands, Ema, Macd, PatternDetector, Rsi, Sma, Stochastic,
    Candle as IndCandle,
};
use arbitragefx::signals::{
    momentum_signal, mean_reversion_signal, trend_signal, trend_aligned_momentum,
    funding_carry_signal, volatility_breakout, climax_signal, Signal,
};
use arbitragefx::sizing::{
    fixed_size, risk_based_size, volatility_adjusted_size, kelly_size,
    signal_scaled_size, apply_max_position, round_to_lot, PositionSizer,
};
use arbitragefx::strategy::{
    Action, Candle, IndicatorSnapshot, MarketAux, MarketView, MetricsState,
    PortfolioState, StrategyState,
};

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

/// Full indicator state
struct IndicatorState {
    ema_fast: Ema,
    ema_slow: Ema,
    rsi: Rsi,
    macd: Macd,
    bb: BollingerBands,
    atr: Atr,
    stoch: Stochastic,
    pattern: PatternDetector,
    price_sma: Sma,
    vol_sma: Sma,
}

impl IndicatorState {
    fn new() -> Self {
        Self {
            ema_fast: Ema::new(12),
            ema_slow: Ema::new(26),
            rsi: Rsi::new(14),
            macd: Macd::default_12_26_9(),
            bb: BollingerBands::default_20_2(),
            atr: Atr::new(14),
            stoch: Stochastic::default_14_3_3(),
            pattern: PatternDetector::new(),
            price_sma: Sma::new(20),
            vol_sma: Sma::new(20),
        }
    }

    fn update(&mut self, row: &CsvRow) -> IndicatorSnapshot {
        let ema_fast = self.ema_fast.update(row.c);
        let ema_slow = self.ema_slow.update(row.c);
        self.rsi.update(row.c);
        self.macd.update(row.c);
        self.bb.update(row.c);
        let atr = self.atr.update(row.h, row.l, row.c);
        self.stoch.update(row.h, row.l, row.c);
        let price_mean = self.price_sma.update(row.c);
        let vol_mean = self.vol_sma.update((row.h - row.l) / row.c);

        let vol = (row.h - row.l) / row.c;
        let momentum = row.c - ema_fast;
        let stretch = row.c - ema_slow;

        // Compute z-scores (simplified)
        let price_std = (self.bb.upper - self.bb.lower) / 4.0; // Approx std from BB
        let z_momentum = if price_std > 0.0 { momentum / price_std } else { 0.0 };
        let z_stretch = if price_std > 0.0 { stretch / price_std } else { 0.0 };
        let z_vol = if vol_mean > 0.0 { (vol - vol_mean) / vol_mean } else { 0.0 };

        // Detect patterns
        self.pattern.update(IndCandle {
            open: row.o,
            high: row.h,
            low: row.l,
            close: row.c,
        });

        IndicatorSnapshot {
            ema_fast,
            ema_slow,
            vwap: price_mean,
            vol,
            vol_mean,
            momentum,
            volume_spike: row.v / 1000.0, // Normalized
            stretch,
            z_momentum,
            z_vol,
            z_volume_spike: 0.0,
            z_stretch,
        }
    }

    fn rsi(&self) -> f64 {
        self.rsi.get()
    }

    fn macd_histogram(&self) -> f64 {
        self.macd.histogram
    }

    fn bb_percent_b(&self, price: f64) -> f64 {
        self.bb.percent_b(price)
    }

    fn atr(&self) -> f64 {
        self.atr.get()
    }

    fn stoch_k(&self) -> f64 {
        self.stoch.k
    }
}

/// Strategy configuration
#[derive(Clone)]
struct LabStrategy {
    name: &'static str,
    // Signal parameters
    use_momentum: bool,
    use_rsi: bool,
    use_macd: bool,
    use_bb: bool,
    use_trend_filter: bool,
    // Thresholds
    momentum_threshold: f64,
    rsi_oversold: f64,
    rsi_overbought: f64,
    // Risk
    stop_loss_atr: f64,
    take_profit_atr: f64,
    max_position_pct: f64,
    // Sizing
    use_vol_sizing: bool,
    base_size: f64,
}

impl LabStrategy {
    fn momentum_basic() -> Self {
        Self {
            name: "momentum_basic",
            use_momentum: true,
            use_rsi: false,
            use_macd: false,
            use_bb: false,
            use_trend_filter: false,
            momentum_threshold: 1.0,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 2.0,
            take_profit_atr: 3.0,
            max_position_pct: 0.05,
            use_vol_sizing: false,
            base_size: 0.001,
        }
    }

    fn momentum_filtered() -> Self {
        Self {
            name: "momentum_filtered",
            use_momentum: true,
            use_rsi: false,
            use_macd: false,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 1.0,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 2.0,
            take_profit_atr: 3.0,
            max_position_pct: 0.05,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    fn rsi_reversal() -> Self {
        Self {
            name: "rsi_reversal",
            use_momentum: false,
            use_rsi: true,
            use_macd: false,
            use_bb: false,
            use_trend_filter: false,
            momentum_threshold: 1.0,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 1.5,
            take_profit_atr: 2.0,
            max_position_pct: 0.03,
            use_vol_sizing: false,
            base_size: 0.001,
        }
    }

    fn macd_crossover() -> Self {
        Self {
            name: "macd_crossover",
            use_momentum: false,
            use_rsi: false,
            use_macd: true,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 1.0,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 2.0,
            take_profit_atr: 3.0,
            max_position_pct: 0.05,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    fn bb_mean_revert() -> Self {
        Self {
            name: "bb_mean_revert",
            use_momentum: false,
            use_rsi: false,
            use_macd: false,
            use_bb: true,
            use_trend_filter: false,
            momentum_threshold: 1.0,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 1.5,
            take_profit_atr: 1.0,
            max_position_pct: 0.03,
            use_vol_sizing: false,
            base_size: 0.001,
        }
    }

    fn multi_indicator() -> Self {
        Self {
            name: "multi_indicator",
            use_momentum: true,
            use_rsi: true,
            use_macd: true,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 0.8,
            rsi_oversold: 35.0,
            rsi_overbought: 65.0,
            stop_loss_atr: 2.0,
            take_profit_atr: 2.5,
            max_position_pct: 0.04,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    fn conservative() -> Self {
        Self {
            name: "conservative",
            use_momentum: true,
            use_rsi: true,
            use_macd: false,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 1.0,  // Relaxed from 1.5
            rsi_oversold: 28.0,       // Relaxed from 25
            rsi_overbought: 72.0,     // Relaxed from 75
            stop_loss_atr: 1.5,
            take_profit_atr: 2.0,
            max_position_pct: 0.02,
            use_vol_sizing: true,
            base_size: 0.0005,
        }
    }

    fn aggressive() -> Self {
        Self {
            name: "aggressive",
            use_momentum: true,
            use_rsi: false,
            use_macd: true,
            use_bb: false,
            use_trend_filter: false,
            momentum_threshold: 0.5,
            rsi_oversold: 40.0,
            rsi_overbought: 60.0,
            stop_loss_atr: 3.0,
            take_profit_atr: 4.0,
            max_position_pct: 0.08,
            use_vol_sizing: false,
            base_size: 0.002,
        }
    }

    /// Trend-following strategy - goes with the trend, not against it
    fn trend_follower() -> Self {
        Self {
            name: "trend_follower",
            use_momentum: true,
            use_rsi: false,
            use_macd: true,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 0.7,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 2.5,
            take_profit_atr: 4.0,  // Let winners run
            max_position_pct: 0.05,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    /// Breakout strategy - buys/sells on volatility expansion
    fn breakout() -> Self {
        Self {
            name: "breakout",
            use_momentum: true,
            use_rsi: false,
            use_macd: false,
            use_bb: true,
            use_trend_filter: false,
            momentum_threshold: 1.2,  // Strong momentum required
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            stop_loss_atr: 1.5,
            take_profit_atr: 3.0,
            max_position_pct: 0.04,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    /// Short-biased for bearish markets
    fn short_bias() -> Self {
        Self {
            name: "short_bias",
            use_momentum: true,
            use_rsi: true,
            use_macd: false,
            use_bb: false,
            use_trend_filter: true,
            momentum_threshold: 0.8,
            rsi_oversold: 25.0,  // Only long on extreme oversold
            rsi_overbought: 60.0,  // Short earlier
            stop_loss_atr: 2.0,
            take_profit_atr: 2.5,
            max_position_pct: 0.04,
            use_vol_sizing: true,
            base_size: 0.001,
        }
    }

    /// Combined signals with confirmation
    fn confluence() -> Self {
        Self {
            name: "confluence",
            use_momentum: true,
            use_rsi: true,
            use_macd: true,
            use_bb: true,
            use_trend_filter: true,
            momentum_threshold: 0.6,
            rsi_oversold: 35.0,
            rsi_overbought: 65.0,
            stop_loss_atr: 1.8,
            take_profit_atr: 2.2,
            max_position_pct: 0.03,
            use_vol_sizing: true,
            base_size: 0.0008,
        }
    }
}

struct LabResult {
    name: &'static str,
    pnl: f64,
    trades: u64,
    wins: u64,
    max_dd: f64,
    sharpe: f64,
}

fn run_strategy(strat: &LabStrategy, rows: &[CsvRow]) -> LabResult {
    let mut ind = IndicatorState::new();
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

    let mut trades = 0u64;
    let mut wins = 0u64;
    let mut returns: Vec<f64> = Vec::new();
    let mut prev_equity = 1000.0;

    for row in rows {
        let indicators = ind.update(row);
        let price = row.c;
        let atr = ind.atr();

        // Calculate signal
        let mut signal_score = 0.0;
        let mut signal_count = 0;

        if strat.use_momentum {
            let mom = momentum_signal(&indicators, strat.momentum_threshold);
            signal_score += mom.direction * mom.strength;
            signal_count += 1;
        }

        if strat.use_rsi {
            let rsi = ind.rsi();
            if rsi < strat.rsi_oversold {
                signal_score += 1.0;
                signal_count += 1;
            } else if rsi > strat.rsi_overbought {
                signal_score -= 1.0;
                signal_count += 1;
            }
        }

        if strat.use_macd {
            let hist = ind.macd_histogram();
            if hist > 0.0 {
                signal_score += hist.min(1.0);
            } else {
                signal_score += hist.max(-1.0);
            }
            signal_count += 1;
        }

        if strat.use_bb {
            let pct_b = ind.bb_percent_b(price);
            if pct_b < 0.0 {
                signal_score += 0.5; // Oversold
            } else if pct_b > 1.0 {
                signal_score -= 0.5; // Overbought
            }
            signal_count += 1;
        }

        let final_signal = if signal_count > 0 {
            signal_score / signal_count as f64
        } else {
            0.0
        };

        // Apply trend filter
        let trend_ok = if strat.use_trend_filter {
            let trend = trend_signal(&indicators);
            (final_signal > 0.0 && trend.is_bullish()) || (final_signal < 0.0 && trend.is_bearish())
        } else {
            true
        };

        // Check exits
        if state.portfolio.position != 0.0 {
            let entry = state.portfolio.entry_price;
            let pnl_pct = if state.portfolio.position > 0.0 {
                (price - entry) / entry
            } else {
                (entry - price) / entry
            };

            let stop = -strat.stop_loss_atr * atr / price;
            let tp = strat.take_profit_atr * atr / price;

            if pnl_pct <= stop || pnl_pct >= tp {
                // Close position
                let qty = -state.portfolio.position;
                let fee = price * qty.abs() * 0.001;
                let realized = if state.portfolio.position > 0.0 {
                    (price - entry) * state.portfolio.position.abs()
                } else {
                    (entry - price) * state.portfolio.position.abs()
                };
                state.metrics.pnl += realized - fee;
                state.portfolio.position = 0.0;
                state.portfolio.cash += realized - fee;
                trades += 1;
                if realized > fee { wins += 1; }
            }
        }

        // Check entries
        if state.portfolio.position == 0.0 && trend_ok && final_signal.abs() > 0.3 {
            let mut size = strat.base_size;

            if strat.use_vol_sizing && indicators.vol_mean > 0.0 {
                size = volatility_adjusted_size(size, 0.02, indicators.vol);
            }

            size = apply_max_position(size, 0.0, strat.max_position_pct);
            size = round_to_lot(size, 0.0001);

            if size > 0.0001 {
                let fee = price * size * 0.001;
                if final_signal > 0.0 {
                    state.portfolio.position = size;
                    state.portfolio.entry_price = price;
                    state.portfolio.cash -= price * size + fee;
                } else {
                    state.portfolio.position = -size;
                    state.portfolio.entry_price = price;
                    state.portfolio.cash += price * size - fee;
                }
                state.metrics.pnl -= fee;
            }
        }

        // Update equity
        state.portfolio.equity = state.portfolio.cash + state.portfolio.position * price;
        if state.portfolio.equity > state.metrics.equity_peak {
            state.metrics.equity_peak = state.portfolio.equity;
        }
        let dd = (state.portfolio.equity - state.metrics.equity_peak) / state.metrics.equity_peak;
        if dd < state.metrics.max_drawdown {
            state.metrics.max_drawdown = dd;
        }

        // Track returns
        let ret = (state.portfolio.equity - prev_equity) / prev_equity;
        returns.push(ret);
        prev_equity = state.portfolio.equity;
    }

    // Close any remaining position
    if let Some(last) = rows.last() {
        if state.portfolio.position != 0.0 {
            let entry = state.portfolio.entry_price;
            let realized = if state.portfolio.position > 0.0 {
                (last.c - entry) * state.portfolio.position.abs()
            } else {
                (entry - last.c) * state.portfolio.position.abs()
            };
            state.metrics.pnl += realized;
            if realized > 0.0 { wins += 1; }
            trades += 1;
        }
    }

    // Calculate Sharpe
    let mean_ret = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean_ret).powi(2)).sum::<f64>() / returns.len() as f64;
    let std = var.sqrt();
    let sharpe = if std > 0.0 { mean_ret / std * (252.0_f64).sqrt() } else { 0.0 };

    LabResult {
        name: strat.name,
        pnl: state.metrics.pnl,
        trades,
        wins,
        max_dd: state.metrics.max_drawdown,
        sharpe,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data_path = args.get(1).map(|s| s.as_str()).unwrap_or("data/btc_1h_180d.csv");

    println!("=== STRATEGY LAB ===\n");
    println!("Loading data from {}...", data_path);

    let rows = load_data(data_path);
    println!("Loaded {} bars\n", rows.len());

    if rows.len() < 100 {
        eprintln!("Not enough data");
        return;
    }

    let first = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last = rows.last().map(|r| r.c).unwrap_or(0.0);
    println!("Price: {:.2} → {:.2} ({:.1}%)\n", first, last, (last - first) / first * 100.0);

    let strategies = vec![
        LabStrategy::momentum_basic(),
        LabStrategy::momentum_filtered(),
        LabStrategy::rsi_reversal(),
        LabStrategy::macd_crossover(),
        LabStrategy::bb_mean_revert(),
        LabStrategy::multi_indicator(),
        LabStrategy::conservative(),
        LabStrategy::aggressive(),
        LabStrategy::trend_follower(),
        LabStrategy::breakout(),
        LabStrategy::short_bias(),
        LabStrategy::confluence(),
    ];

    println!("{:<20} {:>10} {:>8} {:>8} {:>10} {:>8}",
             "Strategy", "PnL", "Trades", "Win%", "MaxDD", "Sharpe");
    println!("{}", "-".repeat(70));

    let mut results = Vec::new();

    for strat in &strategies {
        let start = Instant::now();
        let result = run_strategy(strat, &rows);
        let elapsed = start.elapsed();

        let win_pct = if result.trades > 0 {
            result.wins as f64 / result.trades as f64 * 100.0
        } else {
            0.0
        };

        println!("{:<20} {:>10.2} {:>8} {:>7.1}% {:>9.2}% {:>8.2}",
                 result.name, result.pnl, result.trades, win_pct,
                 result.max_dd * 100.0, result.sharpe);

        results.push(result);
    }

    println!("\n=== Summary ===");
    let profitable: Vec<_> = results.iter().filter(|r| r.pnl > 0.0).collect();
    println!("Profitable: {}/{}", profitable.len(), results.len());

    if let Some(best) = results.iter().max_by(|a, b| a.pnl.partial_cmp(&b.pnl).unwrap()) {
        println!("Best PnL: {} ({:.2})", best.name, best.pnl);
    }
    if let Some(best) = results.iter().max_by(|a, b| a.sharpe.partial_cmp(&b.sharpe).unwrap()) {
        println!("Best Sharpe: {} ({:.2})", best.name, best.sharpe);
    }
    if let Some(best) = results.iter().min_by(|a, b| a.max_dd.partial_cmp(&b.max_dd).unwrap()) {
        println!("Lowest DD: {} ({:.2}%)", best.name, best.max_dd * 100.0);
    }

    // Signal analysis
    println!("\n=== Signal Analysis ===");
    let mut ind = IndicatorState::new();
    let mut bullish = 0;
    let mut bearish = 0;
    let mut neutral = 0;
    let mut trend_up = 0;
    let mut trend_down = 0;

    for row in &rows {
        let indicators = ind.update(row);
        let mom = momentum_signal(&indicators, 1.0);
        let trend = trend_signal(&indicators);

        if mom.is_bullish() { bullish += 1; }
        else if mom.is_bearish() { bearish += 1; }
        else { neutral += 1; }

        if trend.is_bullish() { trend_up += 1; }
        else if trend.is_bearish() { trend_down += 1; }
    }

    println!("Momentum signals: {} bullish, {} bearish, {} neutral",
             bullish, bearish, neutral);
    println!("Trend signals: {} up, {} down", trend_up, trend_down);
    println!("Bull/Bear ratio: {:.2}", bullish as f64 / bearish.max(1) as f64);

    // Market regime analysis
    println!("\n=== Market Regime ===");
    let price_change = (last - first) / first * 100.0;
    if price_change < -20.0 {
        println!("Strong BEAR market ({:.1}%) - short-biased strategies should outperform", price_change);
    } else if price_change < -5.0 {
        println!("Moderate BEAR market ({:.1}%) - trend-following shorts recommended", price_change);
    } else if price_change > 20.0 {
        println!("Strong BULL market ({:.1}%) - momentum longs should work", price_change);
    } else if price_change > 5.0 {
        println!("Moderate BULL market ({:.1}%)", price_change);
    } else {
        println!("RANGE-BOUND market ({:.1}%) - mean reversion strategies may work", price_change);
    }
}
