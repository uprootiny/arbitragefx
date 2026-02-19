//! Research Lab - hypothesis-driven strategy development.
//!
//! Commands:
//!   hypotheses   - List all hypotheses and their status
//!   test H_ID    - Run backtest for a hypothesis
//!   evidence     - Show all evidence
//!   suggest      - Suggest next experiments
//!   report       - Generate full research report

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::hypothesis::{
    BacktestMetrics, Evidence, HypothesisLedger, HypothesisStatus, MarketRegime,
    standard_hypotheses,
};
use arbitragefx::indicators::{Atr, BollingerBands, Ema, Macd, Rsi, Sma};
use arbitragefx::signals::{momentum_signal, mean_reversion_signal, trend_signal};
use arbitragefx::strategy::IndicatorSnapshot;

const LEDGER_PATH: &str = "data/hypothesis_ledger.json";

fn load_ledger() -> HypothesisLedger {
    match std::fs::read_to_string(LEDGER_PATH) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| {
            eprintln!("Failed to parse ledger, creating new");
            create_initial_ledger()
        }),
        Err(_) => create_initial_ledger(),
    }
}

fn save_ledger(ledger: &HypothesisLedger) {
    let json = serde_json::to_string_pretty(ledger).expect("Failed to serialize");
    std::fs::write(LEDGER_PATH, json).expect("Failed to write ledger");
}

fn create_initial_ledger() -> HypothesisLedger {
    let mut ledger = HypothesisLedger::new();
    for h in standard_hypotheses() {
        ledger.add_hypothesis(h);
    }
    ledger
}

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

/// Indicator state for hypothesis testing
struct Indicators {
    ema_fast: Ema,
    ema_slow: Ema,
    rsi: Rsi,
    macd: Macd,
    bb: BollingerBands,
    atr: Atr,
    price_sma: Sma,
    vol_sma: Sma,
}

impl Indicators {
    fn new() -> Self {
        Self {
            ema_fast: Ema::new(12),
            ema_slow: Ema::new(26),
            rsi: Rsi::new(14),
            macd: Macd::default_12_26_9(),
            bb: BollingerBands::default_20_2(),
            atr: Atr::new(14),
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
        self.atr.update(row.h, row.l, row.c);
        let price_mean = self.price_sma.update(row.c);
        let vol_mean = self.vol_sma.update((row.h - row.l) / row.c);

        let vol = (row.h - row.l) / row.c;
        let momentum = row.c - ema_fast;
        let stretch = row.c - ema_slow;

        let price_std = (self.bb.upper - self.bb.lower) / 4.0;
        let z_momentum = if price_std > 0.0 { momentum / price_std } else { 0.0 };
        let z_stretch = if price_std > 0.0 { stretch / price_std } else { 0.0 };
        let z_vol = if vol_mean > 0.0 { (vol - vol_mean) / vol_mean } else { 0.0 };

        IndicatorSnapshot {
            ema_fast,
            ema_slow,
            vwap: price_mean,
            vol,
            vol_mean,
            momentum,
            volume_spike: row.v / 1000.0,
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

    fn atr(&self) -> f64 {
        self.atr.get()
    }
}

/// Test a specific hypothesis
fn test_hypothesis(hypothesis_id: &str, data_path: &str) -> Option<BacktestMetrics> {
    let rows = load_data(data_path);
    if rows.len() < 100 {
        eprintln!("Not enough data");
        return None;
    }

    let start = Instant::now();
    let mut ind = Indicators::new();

    // State for trading simulation
    let mut cash: f64 = 1000.0;
    let mut position: f64 = 0.0;
    let mut entry_price: f64 = 0.0;
    let mut trades = 0u32;
    let mut wins = 0u32;
    let mut total_win = 0.0;
    let mut total_loss = 0.0;
    let mut equity_peak = 1000.0;
    let mut max_dd = 0.0;
    let mut returns: Vec<f64> = Vec::new();
    let mut prev_equity = 1000.0;

    for row in &rows {
        let indicators = ind.update(row);
        let price = row.c;
        let atr = ind.atr();
        let rsi = ind.rsi();

        // Generate signal based on hypothesis
        let (long_signal, short_signal) = match hypothesis_id {
            "H001_momentum" => {
                let mom = momentum_signal(&indicators, 1.0);
                (mom.is_bullish() && mom.strength > 0.5,
                 mom.is_bearish() && mom.strength > 0.5)
            }
            "H002_mean_reversion" => {
                let rev = mean_reversion_signal(&indicators, 2.0);
                (rev.is_bullish() && rev.strength > 0.5,
                 rev.is_bearish() && rev.strength > 0.5)
            }
            "H003_volatility_scaling" => {
                // Test vol-scaled momentum
                let mom = momentum_signal(&indicators, 0.8);
                (mom.is_bullish(), mom.is_bearish())
            }
            "H004_trend_filter" => {
                let mom = momentum_signal(&indicators, 0.8);
                let trend = trend_signal(&indicators);
                (mom.is_bullish() && trend.is_bullish(),
                 mom.is_bearish() && trend.is_bearish())
            }
            "H005_rsi_extremes" => {
                (rsi < 30.0, rsi > 70.0)
            }
            "H006_bear_short" => {
                let trend = trend_signal(&indicators);
                let mom = momentum_signal(&indicators, 0.7);
                // Short bias - more aggressive shorts
                (mom.is_bullish() && trend.is_bullish() && mom.strength > 0.7,
                 mom.is_bearish() || (trend.is_bearish() && mom.strength > 0.3))
            }
            "H007_confluence" => {
                let mom = momentum_signal(&indicators, 0.5);
                let trend = trend_signal(&indicators);
                let rsi_bull = rsi < 40.0;
                let rsi_bear = rsi > 60.0;
                let macd_bull = ind.macd.histogram > 0.0;
                let macd_bear = ind.macd.histogram < 0.0;

                let bull_count = [mom.is_bullish(), trend.is_bullish(), rsi_bull, macd_bull]
                    .iter().filter(|&&x| x).count();
                let bear_count = [mom.is_bearish(), trend.is_bearish(), rsi_bear, macd_bear]
                    .iter().filter(|&&x| x).count();

                (bull_count >= 3, bear_count >= 3)
            }
            "H008_vol_breakout" => {
                // Volatility breakout from low-vol periods
                let vol_ratio = if indicators.vol_mean > 0.0 {
                    indicators.vol / indicators.vol_mean
                } else {
                    1.0
                };
                let trend = trend_signal(&indicators);
                let breakout = vol_ratio > 1.5 && indicators.z_vol > 0.5;
                (breakout && trend.is_bullish(), breakout && trend.is_bearish())
            }
            _ => (false, false),
        };

        // Handle exits
        if position != 0.0 {
            let pnl_pct = if position > 0.0 {
                (price - entry_price) / entry_price
            } else {
                (entry_price - price) / entry_price
            };

            // ATR-based stops
            let stop = -2.0 * atr / price;
            let tp = 2.5 * atr / price;

            if pnl_pct <= stop || pnl_pct >= tp {
                let realized = if position > 0.0 {
                    (price - entry_price) * position.abs()
                } else {
                    (entry_price - price) * position.abs()
                };
                cash += realized;
                trades += 1;
                if realized > 0.0 {
                    wins += 1;
                    total_win += realized;
                } else {
                    total_loss += realized.abs();
                }
                position = 0.0;
            }
        }

        // Handle entries
        if position == 0.0 {
            let mut size = 0.001;

            // Apply volatility scaling for H003
            if hypothesis_id == "H003_volatility_scaling" && indicators.vol_mean > 0.0 {
                let vol_ratio = indicators.vol / indicators.vol_mean;
                size *= (1.0 / vol_ratio).clamp(0.25, 2.0);
            }

            if long_signal {
                position = size;
                entry_price = price;
                cash -= price * size;
            } else if short_signal {
                position = -size;
                entry_price = price;
                cash += price * size;
            }
        }

        // Update equity
        let equity = cash + position * price;
        if equity > equity_peak {
            equity_peak = equity;
        }
        let dd = (equity - equity_peak) / equity_peak;
        if dd < max_dd {
            max_dd = dd;
        }

        let ret = (equity - prev_equity) / prev_equity;
        returns.push(ret);
        prev_equity = equity;
    }

    // Close remaining position
    if let Some(last) = rows.last() {
        if position != 0.0 {
            let realized = if position > 0.0 {
                (last.c - entry_price) * position.abs()
            } else {
                (entry_price - last.c) * position.abs()
            };
            cash += realized;
            trades += 1;
            if realized > 0.0 {
                wins += 1;
                total_win += realized;
            } else {
                total_loss += realized.abs();
            }
        }
    }

    // Calculate metrics
    let final_equity = cash;
    let pnl = final_equity - 1000.0;
    let win_rate = if trades > 0 { wins as f64 / trades as f64 } else { 0.0 };
    let avg_win = if wins > 0 { total_win / wins as f64 } else { 0.0 };
    let avg_loss = if trades > wins { total_loss / (trades - wins) as f64 } else { 0.0 };

    let mean_ret = returns.iter().sum::<f64>() / returns.len().max(1) as f64;
    let var = returns.iter()
        .map(|r| (r - mean_ret).powi(2))
        .sum::<f64>() / returns.len().max(1) as f64;
    let std = var.sqrt();
    let sharpe = if std > 0.0 { mean_ret / std * (252.0_f64).sqrt() } else { 0.0 };

    let profit_factor = if avg_loss > 0.0 { avg_win / avg_loss } else { 0.0 };

    Some(BacktestMetrics {
        pnl,
        trades,
        wins,
        losses: trades - wins,
        win_rate,
        sharpe,
        max_drawdown: max_dd,
        profit_factor,
        avg_win,
        avg_loss,
        expectancy: (win_rate * avg_win) - ((1.0 - win_rate) * avg_loss),
        bars_tested: rows.len() as u64,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

fn cmd_hypotheses(ledger: &HypothesisLedger) {
    println!("=== HYPOTHESIS LEDGER ===\n");

    for status in [
        HypothesisStatus::Supported,
        HypothesisStatus::Testing,
        HypothesisStatus::Proposed,
        HypothesisStatus::Inconclusive,
        HypothesisStatus::Refuted,
    ] {
        let hs = ledger.by_status(status);
        if hs.is_empty() {
            continue;
        }

        println!("{:?} ({}):", status, hs.len());
        for h in hs {
            let evidence_count = ledger.evidence.iter()
                .filter(|e| e.hypothesis_id == h.id)
                .count();
            println!("  {} - {} [{}]",
                     h.id, h.statement,
                     if evidence_count > 0 {
                         format!("{} evidence", evidence_count)
                     } else {
                         "untested".into()
                     });
        }
        println!();
    }
}

fn cmd_test(ledger: &mut HypothesisLedger, hypothesis_id: &str, data_path: &str) {
    if !ledger.hypotheses.contains_key(hypothesis_id) {
        eprintln!("Unknown hypothesis: {}", hypothesis_id);
        return;
    }

    println!("Testing hypothesis: {}", hypothesis_id);
    println!("Data: {}\n", data_path);

    // Load data to get regime
    let rows = load_data(data_path);
    let first = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last = rows.last().map(|r| r.c).unwrap_or(0.0);
    let price_change = (last - first) / first * 100.0;
    let regime = MarketRegime::from_price_change(price_change);

    println!("Price: {:.2} → {:.2} ({:.1}%)", first, last, price_change);
    println!("Regime: {:?}\n", regime);

    if let Some(metrics) = test_hypothesis(hypothesis_id, data_path) {
        // Display results
        println!("Results:");
        println!("  PnL:           {:.2}", metrics.pnl);
        println!("  Trades:        {}", metrics.trades);
        println!("  Win Rate:      {:.1}%", metrics.win_rate * 100.0);
        println!("  Sharpe:        {:.2}", metrics.sharpe);
        println!("  Max Drawdown:  {:.1}%", metrics.max_drawdown * 100.0);
        println!("  Profit Factor: {:.2}", metrics.profit_factor);
        println!("  Expectancy:    {:.4}", metrics.expectancy);

        // Check if meets success criteria
        if let Some(h) = ledger.hypotheses.get(hypothesis_id) {
            let meets = metrics.meets_criteria(&h.success_criteria);
            println!("\nMeets success criteria: {}", if meets { "YES" } else { "NO" });
        }

        // Record evidence
        let evidence = Evidence {
            id: format!("E{}_{}", ledger.evidence.len() + 1, hypothesis_id),
            hypothesis_id: hypothesis_id.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            data_source: data_path.to_string(),
            regime,
            metrics,
            supports_hypothesis: None,  // Will be set by record_evidence
            notes: format!("Auto-generated from backtest on {}", data_path),
        };
        ledger.record_evidence(evidence);
        ledger.update_hypothesis_status(hypothesis_id);

        // Report new status
        if let Some(h) = ledger.hypotheses.get(hypothesis_id) {
            println!("Hypothesis status: {:?}", h.status);
        }

        save_ledger(ledger);
        println!("\nEvidence recorded and ledger saved.");
    } else {
        println!("Test failed!");
    }
}

fn cmd_evidence(ledger: &HypothesisLedger) {
    println!("=== EVIDENCE LOG ===\n");

    if ledger.evidence.is_empty() {
        println!("No evidence recorded yet. Run tests with: research_lab test H_ID");
        return;
    }

    for e in &ledger.evidence {
        let support_str = match e.supports_hypothesis {
            Some(true) => "SUPPORTS",
            Some(false) => "REFUTES",
            None => "UNKNOWN",
        };
        println!("{} → {} [{:?}]",
                 e.id, e.hypothesis_id, e.regime);
        println!("  Sharpe: {:.2} | WinRate: {:.1}% | PnL: {:.2} | {}",
                 e.metrics.sharpe, e.metrics.win_rate * 100.0,
                 e.metrics.pnl, support_str);
    }
}

fn cmd_suggest(ledger: &HypothesisLedger) {
    println!("=== SUGGESTED EXPERIMENTS ===\n");

    let suggestions = ledger.suggest_experiments();

    if suggestions.is_empty() {
        println!("All hypotheses have sufficient evidence.");
        return;
    }

    for s in suggestions {
        if let Some(regime) = s.suggested_regime {
            println!("- Test {} in {:?} regime", s.hypothesis_id, regime);
        } else {
            println!("- {}: {}", s.hypothesis_id, s.reason);
        }
    }
}

fn cmd_report(ledger: &HypothesisLedger, data_path: &str) {
    println!("=== RESEARCH REPORT ===\n");

    let summary = ledger.summary();

    println!("Hypotheses: {}", summary.total_hypotheses);
    for (status, count) in &summary.by_status {
        println!("  {:?}: {}", status, count);
    }

    println!("\nEvidence points: {}", summary.total_evidence);

    if !summary.best_by_regime.is_empty() {
        println!("\nBest performers by regime:");
        for (regime, (hypothesis, sharpe)) in &summary.best_by_regime {
            println!("  {:?}: {} (Sharpe: {:.2})", regime, hypothesis, sharpe);
        }
    }

    // Run all proposed hypotheses if data provided
    if !data_path.is_empty() && std::path::Path::new(data_path).exists() {
        println!("\n=== Running all proposed hypotheses ===\n");

        let proposed: Vec<_> = ledger.by_status(HypothesisStatus::Proposed)
            .iter()
            .map(|h| h.id.clone())
            .collect();

        println!("{:<20} {:>10} {:>8} {:>8} {:>10} {:>8}",
                 "Hypothesis", "PnL", "Trades", "Win%", "Sharpe", "Status");
        println!("{}", "-".repeat(70));

        for h_id in &proposed {
            if let Some(metrics) = test_hypothesis(h_id, data_path) {
                let meets = ledger.hypotheses.get(h_id)
                    .map(|h| metrics.meets_criteria(&h.success_criteria))
                    .unwrap_or(false);

                println!("{:<20} {:>10.2} {:>8} {:>7.1}% {:>10.2} {:>8}",
                         h_id, metrics.pnl, metrics.trades,
                         metrics.win_rate * 100.0, metrics.sharpe,
                         if meets { "PASS" } else { "FAIL" });
            }
        }
    }
}

fn cmd_test_all(ledger: &mut HypothesisLedger, data_path: &str) {
    println!("=== TESTING ALL HYPOTHESES ===\n");

    let rows = load_data(data_path);
    let first = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last = rows.last().map(|r| r.c).unwrap_or(0.0);
    let price_change = (last - first) / first * 100.0;
    let regime = MarketRegime::from_price_change(price_change);

    println!("Data: {} ({} bars)", data_path, rows.len());
    println!("Price: {:.2} → {:.2} ({:.1}%)", first, last, price_change);
    println!("Regime: {:?}\n", regime);

    println!("{:<20} {:>10} {:>8} {:>8} {:>10} {:>10}",
             "Hypothesis", "PnL", "Trades", "Win%", "Sharpe", "Verdict");
    println!("{}", "-".repeat(76));

    let hypothesis_ids: Vec<_> = ledger.hypotheses.keys().cloned().collect();

    for h_id in hypothesis_ids {
        if let Some(metrics) = test_hypothesis(&h_id, data_path) {
            let meets = ledger.hypotheses.get(&h_id)
                .map(|h| metrics.meets_criteria(&h.success_criteria))
                .unwrap_or(false);

            println!("{:<20} {:>10.2} {:>8} {:>7.1}% {:>10.2} {:>10}",
                     h_id, metrics.pnl, metrics.trades,
                     metrics.win_rate * 100.0, metrics.sharpe,
                     if meets { "SUPPORTED" } else { "REFUTED" });

            // Record evidence
            let evidence = Evidence {
                id: format!("E{}_{}", ledger.evidence.len() + 1, h_id),
                hypothesis_id: h_id.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                data_source: data_path.to_string(),
                regime,
                metrics,
                supports_hypothesis: None,
                notes: "Batch test".into(),
            };
            ledger.record_evidence(evidence);
            ledger.update_hypothesis_status(&h_id);
        }
    }

    save_ledger(ledger);
    println!("\nAll evidence recorded and ledger saved.");
}

fn cmd_actions(ledger: &HypothesisLedger) {
    use arbitragefx::hypothesis::DevelopmentAction;

    println!("=== RECOMMENDED ACTIONS ===\n");

    let actions = ledger.derive_actions();

    if actions.is_empty() {
        println!("No actions recommended. Run more tests to generate insights.");
        return;
    }

    for (i, action) in actions.iter().enumerate() {
        match action {
            DevelopmentAction::Refine { hypothesis_id, suggested_change, rationale } => {
                println!("{}. REFINE {}", i + 1, hypothesis_id);
                println!("   Change: {}", suggested_change);
                println!("   Rationale: {}", rationale);
            }
            DevelopmentAction::Combine { hypothesis_ids, combination_logic } => {
                println!("{}. COMBINE {:?}", i + 1, hypothesis_ids);
                println!("   Logic: {}", combination_logic);
            }
            DevelopmentAction::TestRegime { hypothesis_id, regime } => {
                println!("{}. TEST {} in {:?}", i + 1, hypothesis_id, regime);
            }
            DevelopmentAction::Abandon { hypothesis_id, reason } => {
                println!("{}. ABANDON {}", i + 1, hypothesis_id);
                println!("   Reason: {}", reason);
            }
            DevelopmentAction::Promote { hypothesis_id } => {
                println!("{}. PROMOTE {} to production", i + 1, hypothesis_id);
            }
        }
        println!();
    }
}

fn cmd_best(ledger: &HypothesisLedger) {
    use arbitragefx::hypothesis::MarketRegime;

    println!("=== BEST STRATEGIES BY REGIME ===\n");

    for regime in [MarketRegime::StrongBull, MarketRegime::ModerateBull,
                   MarketRegime::Ranging, MarketRegime::ModerateBear,
                   MarketRegime::StrongBear, MarketRegime::HighVolatility] {
        if let Some((h, sharpe)) = ledger.best_for_regime(regime) {
            println!("{:?}:", regime);
            println!("  {} (Sharpe: {:.2})", h.id, sharpe);
            println!("  {}", h.statement);
            println!();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut ledger = load_ledger();

    if args.len() < 2 {
        println!("Research Lab - Hypothesis-driven strategy development\n");
        println!("Commands:");
        println!("  hypotheses              - List all hypotheses");
        println!("  test <H_ID> [data.csv]  - Test a specific hypothesis");
        println!("  test-all [data.csv]     - Test all hypotheses");
        println!("  evidence                - Show evidence log");
        println!("  suggest                 - Suggest next experiments");
        println!("  actions                 - Get recommended development actions");
        println!("  best                    - Show best strategy per regime");
        println!("  report [data.csv]       - Generate research report");
        println!("\nExamples:");
        println!("  research_lab hypotheses");
        println!("  research_lab test H001_momentum data/btc_1h_180d.csv");
        println!("  research_lab test-all data/btc_1h_180d.csv");
        println!("  research_lab actions");
        return;
    }

    let cmd = args[1].as_str();
    let data_path = args.get(3).or(args.get(2))
        .map(|s| s.as_str())
        .unwrap_or("data/btc_1h_180d.csv");

    match cmd {
        "hypotheses" | "h" => cmd_hypotheses(&ledger),
        "test" | "t" => {
            if let Some(h_id) = args.get(2) {
                cmd_test(&mut ledger, h_id, data_path);
            } else {
                eprintln!("Usage: research_lab test <hypothesis_id> [data.csv]");
            }
        }
        "test-all" | "ta" => cmd_test_all(&mut ledger, data_path),
        "evidence" | "e" => cmd_evidence(&ledger),
        "suggest" | "s" => cmd_suggest(&ledger),
        "actions" | "a" => cmd_actions(&ledger),
        "best" | "b" => cmd_best(&ledger),
        "report" | "r" => cmd_report(&ledger, data_path),
        _ => eprintln!("Unknown command: {}", cmd),
    }
}
