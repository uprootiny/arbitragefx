use std::fs::File;
use std::io::{BufRead, BufReader};

use arbitragefx::backtest::parse_csv_line;
use arbitragefx::events::{detect_phase1, EventConfig, EventType};
use arbitragefx::features::FeaturePipeline;
use arbitragefx::state::Config;

#[derive(Debug, Clone)]
struct Trade {
    pub entry_ts: u64,
    pub exit_deadline: u64,
    pub dir: f64,
    pub entry_price: f64,
    pub kind: EventType,
}

#[derive(Debug, Clone)]
struct ExecConfig {
    pub slippage_k: f64,
    pub fee_rate: f64,
    pub latency_min: u64,
    pub latency_max: u64,
}

impl ExecConfig {
    fn from_env() -> Self {
        Self {
            slippage_k: std::env::var("SLIP_K")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0008),
            fee_rate: std::env::var("FEE_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.001),
            latency_min: std::env::var("LAT_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            latency_max: std::env::var("LAT_MAX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8),
        }
    }
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

fn hold_for(kind: &EventType) -> u64 {
    match kind {
        EventType::FundingImbalance => std::env::var("FUNDING_HOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(12),
        EventType::LiquidationCascade => std::env::var("LIQ_HOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6),
        EventType::StablecoinDepeg => std::env::var("DEPEG_HOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24),
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data.csv".to_string());
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("failed to open {}: {}", path, err);
            return;
        }
    };
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if line.to_lowercase().starts_with("ts,") {
            continue;
        }
        match parse_csv_line(&line) {
            Ok(r) => rows.push(r),
            Err(err) => eprintln!("bad row: {}", err),
        }
    }
    if rows.is_empty() {
        eprintln!("no rows parsed");
        return;
    }

    let cfg = Config::from_env();
    let exec = ExecConfig::from_env();
    let event_cfg = EventConfig::from_env();
    let mut pipeline = FeaturePipeline::new(200, 200, 30, 200);
    let mut active: Vec<Trade> = Vec::new();
    let mut pnl_by_kind = std::collections::HashMap::new();
    let mut count_by_kind = std::collections::HashMap::new();

    for row in rows.iter() {
        let features = pipeline.update(row.c, row.funding, row.oi, row.liq, row.depeg);
        let events = detect_phase1(row.ts, &features, &event_cfg);

        for evt in events {
            let hold = hold_for(&evt.event);
            let dir = match evt.event {
                EventType::FundingImbalance => {
                    if features.funding_rate > 0.0 {
                        -1.0
                    } else {
                        1.0
                    }
                }
                EventType::LiquidationCascade => {
                    if features.price_velocity >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                EventType::StablecoinDepeg => {
                    if features.stable_depeg < 0.0 {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            let delay = latency_step(row.ts, exec.latency_min, exec.latency_max);
            let entry_ts = row.ts + delay;
            active.push(Trade {
                entry_ts,
                exit_deadline: entry_ts + hold * cfg.candle_granularity,
                dir,
                entry_price: row.c,
                kind: evt.event.clone(),
            });
        }

        let mut remaining = Vec::new();
        for trade in active.drain(..) {
            if row.ts < trade.entry_ts {
                remaining.push(trade);
                continue;
            }
            let mut exit_now = row.ts >= trade.exit_deadline;
            if let EventType::StablecoinDepeg = trade.kind {
                if row.depeg.abs() < 0.0005 {
                    exit_now = true;
                }
            }
            if !exit_now {
                remaining.push(trade);
                continue;
            }

            let qty = trade.dir * 1.0;
            let entry = slippage_price(
                trade.entry_price,
                qty,
                row.v,
                exec.slippage_k,
                features.vol_ratio,
            );
            let exit = slippage_price(row.c, -qty, row.v, exec.slippage_k, features.vol_ratio);
            let fee = (entry.abs() + exit.abs()) * exec.fee_rate;
            let pnl = (exit - entry) * qty - fee;
            *pnl_by_kind.entry(trade.kind.clone()).or_insert(0.0) += pnl;
            *count_by_kind.entry(trade.kind.clone()).or_insert(0u64) += 1;
        }
        active = remaining;
    }

    println!("event,pnl,count,avg_pnl");
    for (k, pnl) in pnl_by_kind.iter() {
        let count = *count_by_kind.get(k).unwrap_or(&1);
        println!("{:?},{:.6},{}, {:.6}", k, pnl, count, pnl / count as f64);
    }
}
