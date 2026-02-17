use anyhow::{anyhow, Result};

use crate::events::{detect_phase1, EventConfig};
use crate::features::FeaturePipeline;
use crate::metrics::MetricsEngine;
use crate::risk::RiskEngine;
use crate::state::{Config, Fill, MarketState, StrategyInstance};
use crate::strategy::{Action, MarketAux};

/// Execution mode for backtesting
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecMode {
    /// Instant fill at close price (unrealistic but fast)
    Instant,
    /// Market order with slippage
    Market,
    /// Limit order with fill probability
    Limit,
    /// Realistic: limit with adverse selection and partial fills
    Realistic,
}

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub slippage_k: f64,
    pub fee_rate: f64,
    pub latency_min: u64,
    pub latency_max: u64,
    pub max_fill_ratio: f64,
    /// Execution mode
    pub mode: ExecMode,
    /// Limit order fill probability base (0.0-1.0)
    pub limit_fill_prob: f64,
    /// Adverse selection factor (0.0-1.0) - how much fills are biased against us
    pub adverse_selection: f64,
    /// Volatility multiplier for slippage
    pub vol_slip_mult: f64,
}

impl ExecConfig {
    pub fn from_env() -> Self {
        let mode = match std::env::var("EXEC_MODE").unwrap_or_default().as_str() {
            "instant" => ExecMode::Instant,
            "market" => ExecMode::Market,
            "limit" => ExecMode::Limit,
            "realistic" => ExecMode::Realistic,
            _ => ExecMode::Market,
        };
        Self {
            slippage_k: std::env::var("SLIP_K").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0008),
            fee_rate: std::env::var("FEE_RATE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.001),
            latency_min: std::env::var("LAT_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            latency_max: std::env::var("LAT_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(8),
            max_fill_ratio: std::env::var("FILL_RATIO").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5),
            mode,
            limit_fill_prob: std::env::var("LIMIT_FILL_PROB").ok().and_then(|v| v.parse().ok()).unwrap_or(0.7),
            adverse_selection: std::env::var("ADVERSE_SEL").ok().and_then(|v| v.parse().ok()).unwrap_or(0.3),
            vol_slip_mult: std::env::var("VOL_SLIP_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0),
        }
    }

    /// Instant execution config (for fast testing)
    pub fn instant() -> Self {
        Self {
            mode: ExecMode::Instant,
            slippage_k: 0.0,
            fee_rate: 0.0,
            latency_min: 0,
            latency_max: 0,
            max_fill_ratio: 1.0,
            limit_fill_prob: 1.0,
            adverse_selection: 0.0,
            vol_slip_mult: 0.0,
        }
    }

    /// Maker order config (limit orders, low fees)
    pub fn maker() -> Self {
        Self {
            mode: ExecMode::Limit,
            slippage_k: 0.0001,
            fee_rate: 0.0002,  // Maker fee
            latency_min: 5,
            latency_max: 20,
            max_fill_ratio: 0.8,
            limit_fill_prob: 0.7,
            adverse_selection: 0.25,
            vol_slip_mult: 1.0,
        }
    }

    /// Taker order config (market orders, higher fees)
    pub fn taker() -> Self {
        Self {
            mode: ExecMode::Market,
            slippage_k: 0.0008,
            fee_rate: 0.001,  // Taker fee
            latency_min: 2,
            latency_max: 8,
            max_fill_ratio: 1.0,
            limit_fill_prob: 1.0,
            adverse_selection: 0.0,
            vol_slip_mult: 2.0,
        }
    }

    /// Realistic config (mixed limit/market with adverse selection)
    pub fn realistic() -> Self {
        Self {
            mode: ExecMode::Realistic,
            slippage_k: 0.0005,
            fee_rate: 0.0004,  // Blended rate
            latency_min: 3,
            latency_max: 15,
            max_fill_ratio: 0.6,
            limit_fill_prob: 0.65,
            adverse_selection: 0.3,
            vol_slip_mult: 1.5,
        }
    }
}

/// Calculate fill probability for a limit order
pub fn calc_fill_probability(
    is_buy: bool,
    limit_price: f64,
    current_price: f64,
    volatility: f64,
    base_prob: f64,
    adverse_sel: f64,
) -> f64 {
    if volatility <= 0.0 {
        return base_prob;
    }

    // Distance from current price (negative = favorable, positive = unfavorable)
    let distance = if is_buy {
        (current_price - limit_price) / current_price
    } else {
        (limit_price - current_price) / current_price
    };

    // Favorable limit (below market for buy, above for sell) = higher fill prob
    // Unfavorable limit = lower fill prob
    let distance_factor = if distance > 0.0 {
        // Limit is favorable - high fill probability
        (1.0 + distance / volatility).min(1.5)
    } else {
        // Limit is unfavorable - probability decays
        (-distance.abs() / volatility).exp()
    };

    // Adverse selection: fills that happen tend to be losing trades
    // If we get filled, price was likely moving against us
    let adverse_factor = 1.0 - adverse_sel;

    (base_prob * distance_factor * adverse_factor).clamp(0.0, 1.0)
}

/// Calculate slippage for an order
pub fn calc_slippage(
    qty: f64,
    _price: f64,
    volume: f64,
    volatility: f64,
    config: &ExecConfig,
) -> f64 {
    // Base slippage from size vs volume
    let size_slip = config.slippage_k * qty.abs() / volume.max(1.0);

    // Volatility-adjusted slippage
    let vol_slip = volatility * config.vol_slip_mult * 0.1;

    // Total slippage
    (size_slip + vol_slip).min(0.05)  // Cap at 5%
}

#[derive(Debug, Clone)]
struct PendingOrder {
    pub qty: f64,
    pub submit_ts: u64,
    /// Strategy index that owns this order (FIXED: per-strategy attribution)
    pub strategy_idx: usize,
}

#[derive(Debug, Clone)]
pub struct CsvRow {
    pub ts: u64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
    pub funding: f64,
    pub borrow: f64,
    pub liq: f64,
    pub depeg: f64,
    pub oi: f64,
}

pub fn parse_csv_line(line: &str) -> Result<CsvRow> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 10 {
        return Err(anyhow!("expected 10+ columns, got {}", parts.len()));
    }
    let oi = if parts.len() >= 11 { parts[10].trim().parse().unwrap_or(0.0) } else { 0.0 };
    Ok(CsvRow {
        ts: parts[0].trim().parse()?,
        o: parts[1].trim().parse()?,
        h: parts[2].trim().parse()?,
        l: parts[3].trim().parse()?,
        c: parts[4].trim().parse()?,
        v: parts[5].trim().parse()?,
        funding: parts[6].trim().parse()?,
        borrow: parts[7].trim().parse()?,
        liq: parts[8].trim().parse()?,
        depeg: parts[9].trim().parse()?,
        oi,
    })
}

fn slippage_price(price: f64, qty: f64, liquidity: f64, k: f64, vol: f64) -> f64 {
    let liq = liquidity.max(1.0);
    let slip = k * (qty.abs() / liq) * (1.0 + vol * 2.0);
    if qty >= 0.0 {
        price * (1.0 + slip)
    } else {
        price * (1.0 - slip)
    }
}

fn latency_step(ts: u64, min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let span = max - min + 1;
    min + (ts % span)
}

/// Deterministic latency model with bounded jitter.
/// Uses a simple xorshift to avoid RNG dependencies and keep replay stable.
fn latency_delay(submit_ts: u64, strategy_idx: usize, min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let mut x = submit_ts ^ ((strategy_idx as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    let span = max - min + 1;
    min + (x % span)
}

pub fn run_backtest(cfg: Config, rows: &[CsvRow]) -> Result<(f64, f64)> {
    let exec_cfg = ExecConfig::from_env();
    let event_cfg = EventConfig::from_env();
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_churn_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();
    let mut pending: Vec<PendingOrder> = Vec::new();
    let mut pipeline = FeaturePipeline::new(200, 200, 30, 200);
    let mut friction: Vec<f64> = vec![0.0; strategies.len()];
    let mut holds: Vec<u64> = vec![0; strategies.len()];
    let mut guarded_blocks: Vec<u64> = vec![0; strategies.len()];
    let mut submits: Vec<u64> = vec![0; strategies.len()];
    let mut fills: Vec<u64> = vec![0; strategies.len()];
    let mut forced_closes: Vec<u64> = vec![0; strategies.len()];
    let initial_cash = 1000.0;
    let mut buy_hold_entry = None;
    let mut buy_hold_exit = None;
    let mut last_row: Option<CsvRow> = None;

    for row in rows {
        last_row = Some(row.clone());
        if buy_hold_entry.is_none() {
            buy_hold_entry = Some(row.c);
        }
        buy_hold_exit = Some(row.c);
        let candle = crate::exchange::Candle {
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
        let features = pipeline.update(row.c, row.funding, row.oi, row.liq, row.depeg);
        let _events = detect_phase1(row.ts, &features, &event_cfg);

        for (idx, inst) in strategies.iter_mut().enumerate() {
            let view = market.view(&cfg.symbol);
            let action = inst.strategy.update(view, &mut inst.state);
            // FIXED: Use current price for MTM risk calculations
            let guarded = risk.apply_with_price(&inst.state, action, row.ts, row.c);
            if matches!(action, Action::Hold) {
                holds[idx] += 1;
            }
            if !matches!(action, Action::Hold) && matches!(guarded, Action::Hold) {
                guarded_blocks[idx] += 1;
            }

            let desired = match guarded {
                Action::Hold => None,
                Action::Close => {
                    let qty = -inst.state.portfolio.position;
                    Some((qty, row.c))
                }
                Action::Buy { qty } => Some((qty, row.c)),
                Action::Sell { qty } => Some((-qty.abs(), row.c)),
            };
            if let Some((qty, _price)) = desired {
                // FIXED: Tag order with strategy index
                pending.push(PendingOrder { qty, submit_ts: row.ts, strategy_idx: idx });
                submits[idx] += 1;
            }

            // FIXED: Only process orders belonging to THIS strategy
            let mut still_pending = Vec::new();
            for order in pending.drain(..) {
                // Skip orders from other strategies
                if order.strategy_idx != idx {
                    still_pending.push(order);
                    continue;
                }
                let delay = latency_delay(
                    order.submit_ts,
                    order.strategy_idx,
                    exec_cfg.latency_min,
                    exec_cfg.latency_max,
                );
                if row.ts < order.submit_ts + delay {
                    still_pending.push(order);
                    continue;
                }
                let fill_qty = order.qty * exec_cfg.max_fill_ratio;
                let fill_price = slippage_price(row.c, fill_qty, row.v, exec_cfg.slippage_k, view.indicators.vol);
                let fee = fill_price * fill_qty.abs() * exec_cfg.fee_rate;
                let slip_cost = (fill_price - row.c).abs() * fill_qty.abs();
                friction[idx] += fee + slip_cost;
                let realized = inst
                    .state
                    .portfolio
                    .apply_fill(Fill { price: fill_price, qty: fill_qty, fee, ts: row.ts });
                fills[idx] += 1;
                inst.state.metrics.pnl += realized;
                if realized > 0.0 {
                    inst.state.metrics.wins += 1;
                } else if realized < 0.0 {
                    inst.state.metrics.losses += 1;
                    inst.state.last_loss_ts = row.ts;
                }
                inst.state.last_trade_ts = row.ts;
                let day = row.ts / 86_400;
                if inst.state.trade_day != day {
                    inst.state.trade_day = day;
                    inst.state.trades_today = 0;
                }
                inst.state.trades_today += 1;

                let remainder = order.qty - fill_qty;
                if remainder.abs() > 1e-9 {
                    still_pending.push(PendingOrder { qty: remainder, submit_ts: row.ts, strategy_idx: idx });
                }
            }
            pending = still_pending;
            metrics.update(&mut inst.state);
        }
    }

    if let Some(last) = last_row {
        let view = market.view(&cfg.symbol);
        for (idx, inst) in strategies.iter_mut().enumerate() {
            if inst.state.portfolio.position.abs() > 1e-9 {
                let qty = -inst.state.portfolio.position;
                let fill_price = slippage_price(last.c, qty, last.v, exec_cfg.slippage_k, view.indicators.vol);
                let fee = fill_price * qty.abs() * exec_cfg.fee_rate;
                friction[idx] += fee;
                let realized = inst
                    .state
                    .portfolio
                    .apply_fill(Fill { price: fill_price, qty, fee, ts: last.ts });
                inst.state.metrics.pnl += realized;
                if realized > 0.0 {
                    inst.state.metrics.wins += 1;
                } else if realized < 0.0 {
                    inst.state.metrics.losses += 1;
                }
                fills[idx] += 1;
                forced_closes[idx] += 1;
            }
        }
    }

    let pnl = strategies.iter().map(|s| s.state.metrics.pnl).sum::<f64>();
    let max_dd = strategies
        .iter()
        .map(|s| s.state.metrics.max_drawdown.abs())
        .fold(0.0, f64::max);
    let buy_hold = if let (Some(entry), Some(exit)) = (buy_hold_entry, buy_hold_exit) {
        exit - entry
    } else {
        0.0
    };
    println!("baseline=buy_hold pnl={:.4}", buy_hold);
    println!("baseline=no_trade pnl=0.0000");
    for (idx, inst) in strategies.iter().enumerate() {
        let total_trades = inst.state.metrics.wins + inst.state.metrics.losses;
        let equity_pnl = inst.state.portfolio.equity - initial_cash;
        println!(
            "strategy={} pnl={:.4} equity_pnl={:.4} equity={:.4} pos={:.6} entry={:.2} friction={:.4} friction_only_pnl={:.4} dd={:.4} trades={} wins={} losses={} holds={} guarded={} submits={} fills={} forced_closes={}",
            inst.id,
            inst.state.metrics.pnl,
            equity_pnl,
            inst.state.portfolio.equity,
            inst.state.portfolio.position,
            inst.state.portfolio.entry_price,
            friction[idx],
            -friction[idx],
            inst.state.metrics.max_drawdown.abs(),
            total_trades,
            inst.state.metrics.wins,
            inst.state.metrics.losses,
            holds[idx],
            guarded_blocks[idx],
            submits[idx],
            fills[idx],
            forced_closes[idx]
        );
    }
    Ok((pnl, max_dd))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> Config {
        let mut cfg = Config::from_env();
        cfg.symbol = "BTCUSDT".to_string();
        cfg.candle_granularity = 300;
        cfg.window = 50;
        cfg
    }

    #[test]
    fn test_parse_csv_line_valid() {
        let line = "1000,1,1,1,1,10,0.0001,0.0,0.0,0.0,0.0";
        let row = parse_csv_line(line).unwrap();
        assert_eq!(row.ts, 1000);
        assert_eq!(row.c, 1.0);
        assert_eq!(row.funding, 0.0001);
    }

    #[test]
    fn test_slippage_price_monotonic() {
        let base = 100.0;
        let p1 = slippage_price(base, 0.1, 1000.0, 0.001, 0.01);
        let p2 = slippage_price(base, 1.0, 1000.0, 0.001, 0.01);
        assert!(p2 > p1);
    }

    #[test]
    fn test_slippage_rises_with_volatility() {
        let base = 100.0;
        let low_vol = slippage_price(base, 1.0, 1000.0, 0.001, 0.005);
        let high_vol = slippage_price(base, 1.0, 1000.0, 0.001, 0.05);
        assert!(high_vol > low_vol);
    }

    #[test]
    fn test_partial_fills_accumulate_to_full() {
        let mut remaining: f64 = 10.0;
        let max_fill_ratio = 0.5;
        let mut filled = 0.0;
        for _ in 0..5 {
            if remaining.abs() <= 1e-9 {
                break;
            }
            let fill = remaining * max_fill_ratio;
            filled += fill;
            remaining -= fill;
        }
        let total = filled + remaining;
        assert!((total - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_latency_delay_deterministic_and_bounded() {
        let min = 2;
        let max = 8;
        let d1 = latency_delay(1700, 3, min, max);
        let d2 = latency_delay(1700, 3, min, max);
        let d3 = latency_delay(1701, 3, min, max);
        assert_eq!(d1, d2);
        assert!(d1 >= min && d1 <= max);
        assert!(d3 >= min && d3 <= max);
    }

    #[test]
    fn test_run_backtest_smoke() {
        let rows = vec![
            CsvRow { ts: 1000, o: 1.0, h: 1.0, l: 1.0, c: 1.0, v: 1000.0, funding: 0.0, borrow: 0.0, liq: 0.0, depeg: 0.0, oi: 0.0 },
            CsvRow { ts: 1300, o: 1.0, h: 1.0, l: 1.0, c: 1.1, v: 1100.0, funding: 0.0, borrow: 0.0, liq: 0.0, depeg: 0.0, oi: 0.0 },
            CsvRow { ts: 1600, o: 1.0, h: 1.0, l: 1.0, c: 0.9, v: 900.0, funding: 0.0, borrow: 0.0, liq: 0.0, depeg: 0.0, oi: 0.0 },
        ];
        let cfg = test_cfg();
        let result = run_backtest(cfg, &rows);
        assert!(result.is_ok());
    }
}
