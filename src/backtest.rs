use anyhow::{anyhow, Result};

use crate::events::{detect_phase1, EventConfig};
use crate::features::FeaturePipeline;
use crate::metrics::MetricsEngine;
use crate::risk::RiskEngine;
use crate::state::{Config, Fill, MarketState, StrategyInstance};
use crate::strategy::{Action, MarketAux};

#[derive(Debug, Clone)]
pub struct ExecConfig {
    pub slippage_k: f64,
    pub fee_rate: f64,
    pub latency_min: u64,
    pub latency_max: u64,
    pub max_fill_ratio: f64,
}

impl ExecConfig {
    pub fn from_env() -> Self {
        Self {
            slippage_k: std::env::var("SLIP_K").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0008),
            fee_rate: std::env::var("FEE_RATE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.001),
            latency_min: std::env::var("LAT_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(2),
            latency_max: std::env::var("LAT_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(8),
            max_fill_ratio: std::env::var("FILL_RATIO").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5),
        }
    }
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
                let delay = latency_step(order.submit_ts, exec_cfg.latency_min, exec_cfg.latency_max);
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
        .map(|s| s.state.metrics.max_drawdown)
        .fold(0.0, f64::min);
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
            inst.state.metrics.max_drawdown,
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
