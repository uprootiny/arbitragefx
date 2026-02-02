use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;

use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, MarketState, StrategyInstance};
use arbitragefx::strategy::Action;
use arbitragefx::engine::experiment_registry::{
    CorrectionMethod, ExperimentRegistry, ExperimentRun, TrialMetrics, TrialResult, TrialStatus,
};

#[derive(Clone, Copy)]
struct TrialConfig {
    ema_fast: u32,
    ema_slow: u32,
    entry_th: f64,
    edge_scale: f64,
}

fn generate_candles(seed: u64, n: usize, start_price: f64) -> Vec<arbitragefx::exchange::Candle> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut price = start_price;
    let mut vol: f64 = 0.002;
    let mut series = Vec::with_capacity(n);
    for i in 0..n {
        let regime_flip: f64 = rng.gen();
        if regime_flip < 0.02 {
            vol = (vol * 1.4).min(0.02);
        } else if regime_flip > 0.98 {
            vol = (vol * 0.7).max(0.0008);
        }
        let drift = if i % 240 < 120 { 0.0002 } else { -0.0001 };
        let noise: f64 = rng.gen_range(-1.0..1.0) * vol;
        let ret = drift + noise;
        price = (price * (1.0 + ret)).max(1.0);
        let o = price * (1.0 - vol * 0.2);
        let c = price;
        let h = price * (1.0 + vol * 0.5);
        let l = price * (1.0 - vol * 0.5);
        let v = rng.gen_range(50.0..200.0) * (1.0 + vol * 10.0);
        series.push(arbitragefx::exchange::Candle {
            ts: i as u64 * 300,
            o,
            h,
            l,
            c,
            v,
        });
    }
    series
}

fn run_trial(cfg: &Config, trial: TrialConfig, candles: &[arbitragefx::exchange::Candle]) -> (f64, f64, u32, u32) {
    let mut cfg = cfg.clone();
    cfg.ema_fast = trial.ema_fast;
    cfg.ema_slow = trial.ema_slow;
    cfg.entry_threshold = trial.entry_th;
    cfg.edge_scale = trial.edge_scale;

    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_default_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();

    for candle in candles.iter() {
        market.on_candle(*candle);
        for inst in strategies.iter_mut() {
            let view = market.view(&cfg.symbol);
            let action = inst.strategy.update(view, &mut inst.state);
            let guarded = risk.apply_with_price(&inst.state, action, candle.ts, candle.c);
            let fill = match guarded {
                Action::Hold => None,
                Action::Close => {
                    let qty = -inst.state.portfolio.position;
                    Some((qty, candle.c))
                }
                Action::Buy { qty } => Some((qty, candle.c)),
                Action::Sell { qty } => Some((-qty.abs(), candle.c)),
            };
            if let Some((qty, price)) = fill {
                let fee = price * qty.abs() * 0.001;
                let realized = inst
                    .state
                    .portfolio
                    .apply_fill(arbitragefx::state::Fill { price, qty, fee, ts: candle.ts });
                inst.state.metrics.pnl += realized;
                if realized > 0.0 {
                    inst.state.metrics.wins += 1;
                } else if realized < 0.0 {
                    inst.state.metrics.losses += 1;
                    inst.state.last_loss_ts = candle.ts;
                }
                inst.state.last_trade_ts = candle.ts;
                let day = candle.ts / 86_400;
                if inst.state.trade_day != day {
                    inst.state.trade_day = day;
                    inst.state.trades_today = 0;
                }
                inst.state.trades_today += 1;
            }
            metrics.update(&mut inst.state);
        }
    }

    let pnl = strategies.iter().map(|s| s.state.metrics.pnl).sum::<f64>();
    let max_dd = strategies
        .iter()
        .map(|s| s.state.metrics.max_drawdown)
        .fold(0.0, f64::min);
    let wins: u32 = strategies.iter().map(|s| s.state.metrics.wins as u32).sum();
    let losses: u32 = strategies.iter().map(|s| s.state.metrics.losses as u32).sum();
    (pnl, max_dd, wins, losses)
}

fn now_ts() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn main() -> Result<()> {
    let base = Config::from_env();
    let candles = generate_candles(42, 2000, 30_000.0);

    let ema_fast = [4, 6, 8];
    let ema_slow = [16, 24, 36];
    let entry = [0.8, 1.2, 1.6];
    let edge = [0.0015, 0.0025, 0.0035];

    let planned_trials = (ema_fast.len() * ema_slow.len() * entry.len() * edge.len()) as u32;
    let run_id = format!("trial_{}", now_ts()?);
    let correction = match std::env::var("CORRECTION").unwrap_or_else(|_| "bonferroni".to_string()).to_lowercase().as_str() {
        "none" => CorrectionMethod::None,
        "holm" => CorrectionMethod::Holm,
        "fdr" => CorrectionMethod::FDR,
        "twostage" => CorrectionMethod::TwoStage,
        _ => CorrectionMethod::Bonferroni,
    };
    let out_dir = PathBuf::from("out/experiments");
    let mut registry = ExperimentRegistry::new(&out_dir)?;

    let mut cfg_hasher = std::collections::hash_map::DefaultHasher::new();
    base.symbol.hash(&mut cfg_hasher);
    base.candle_granularity.hash(&mut cfg_hasher);
    base.window.hash(&mut cfg_hasher);
    base.entry_threshold.to_bits().hash(&mut cfg_hasher);
    base.edge_scale.to_bits().hash(&mut cfg_hasher);
    let config_hash = cfg_hasher.finish();

    let run = ExperimentRun {
        id: run_id.clone(),
        git_sha: None,
        start_ts: now_ts()?,
        seed: 42,
        dataset_id: "synthetic".to_string(),
        config_hash,
        description: "parameter sweep".to_string(),
        correction_method: correction,
        planned_trials,
    };
    registry.start_run(run.clone())?;

    let mut results = Vec::new();
    let mut trial_id: u32 = 0;
    for f in ema_fast {
        for s in ema_slow {
            for th in entry {
                for e in edge {
                    let trial = TrialConfig {
                        ema_fast: f,
                        ema_slow: s,
                        entry_th: th,
                        edge_scale: e,
                    };
                    let (pnl, dd, wins, losses) = run_trial(&base, trial, &candles);
                    let total_trades = wins + losses;
                    let win_rate = if total_trades > 0 { wins as f64 / total_trades as f64 } else { 0.0 };
                    let total_return_pct = (pnl / (1000.0 * 3.0)) * 100.0;
                    let max_drawdown_pct = dd.abs() * 100.0;
                    let reality_score = total_return_pct - max_drawdown_pct;

                    let mut params = HashMap::new();
                    params.insert("ema_fast".to_string(), f as f64);
                    params.insert("ema_slow".to_string(), s as f64);
                    params.insert("entry_th".to_string(), th);
                    params.insert("edge_scale".to_string(), e);

                    let metrics = TrialMetrics {
                        total_return_pct,
                        sharpe_ratio: 0.0,
                        max_drawdown_pct,
                        total_trades,
                        win_rate,
                        profit_factor: 0.0,
                        reality_score,
                        extra: HashMap::new(),
                    };

                    let trial_result = TrialResult {
                        run_id: run_id.clone(),
                        trial_id,
                        params,
                        metrics,
                        status: TrialStatus::Completed,
                        notes: Vec::new(),
                        timestamp: now_ts()?,
                    };
                    registry.record_trial(trial_result)?;
                    results.push((trial, pnl, dd));
                    trial_id += 1;
                }
            }
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    println!("ema_fast,ema_slow,entry_th,edge_scale,pnl,max_drawdown");
    for (trial, pnl, dd) in results.iter().take(20) {
        println!("{},{},{},{:.4},{:.2},{:.4}", trial.ema_fast, trial.ema_slow, trial.entry_th, trial.edge_scale, pnl, dd);
    }

    if let Err(err) = registry.finish_run(0.05) {
        eprintln!("[WARN] finish_run failed: {}", err);
    }
    Ok(())
}
