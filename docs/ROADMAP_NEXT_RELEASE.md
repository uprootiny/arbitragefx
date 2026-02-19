# Roadmap: v0.3.0 — Viable Backtesting & Hypothesis Reconciliation System

**Goal:** ArbitrageFX becomes a usable system for backtesting strategies on real data,
evaluating hypotheses with Bayesian truth values, and producing honest assessments
of trading viability across market regimes.

**Updated:** 2026-02-19
**Previous:** v0.2.0 focused on smoke tests & reality grounding (largely complete)

---

## v0.2.0 Status: What's Done

| Phase | Status | Evidence |
|-------|--------|----------|
| Phase 0: Smoke test harness | DONE | 13 Rust smoke tests + 8 shell smoke tests, all green |
| Phase 1a: Epistemic server real data | DONE | Reads hypothesis_ledger.edn, scans data/, reports real pipeline |
| Phase 1b: Config snapshotting | NOT DONE | Still 54 env vars with no snapshot |
| Phase 1c: Data manifest enforcement | PARTIAL | SHA256 hashing works, no enforcement gate |
| Phase 2a: Strategy unit tests | DONE | 15 SimpleMomentum tests covering all decision branches |
| Phase 2b: Regression snapshots | NOT DONE | |
| Phase 2c: Composable strategies resolved | NOT DONE | 808 lines still dead |
| Phase 3a: min_hold_candles | DONE | Wired into SimpleMomentum + CarryOpportunistic |
| Phase 3b: Signal smoothing | NOT DONE | |
| Phase 3c: Re-run hypothesis ledger | DONE | v1.1.0 with 9 hypotheses, 5 datasets |
| Phase 4a: Data fetch script | DONE | scripts/fetch_data.sh |
| Phase 4b: Pipeline script | DONE | scripts/pipeline.sh → dated reports |
| Phase 4c: Cron-ready | PARTIAL | Idempotent but no cron setup |

**Test count:** 353 (up from 258 at v0.1.0)
**Datasets:** 5 real BTC/USDT regime files from Binance public API
**Hypotheses:** 9, with Bayesian truth values updated from 5-regime evidence

---

## v0.3.0: The System Vision

ArbitrageFX should be a **viable backtesting / trading reconnaissance / strategy
evaluation / hypothesis tracking and reconciliation system**. This means:

1. **Backtesting**: Run any strategy on any dataset, get structured results
2. **Reconnaissance**: Fetch new data, detect regime, position in known parameter space
3. **Strategy evaluation**: Compare strategies across regimes with honest friction
4. **Hypothesis tracking**: Bayesian truth values updated from evidence, not narrative
5. **Reconciliation**: When new data arrives, update beliefs and report what changed

---

## Phase 5: Config Reproducibility (2 commits)

### 5a. Config snapshot on every run
- `BacktestResult` includes serialized config JSON
- Each run writes `out/configs/{timestamp}.json`
- Smoke test: two runs with different ENV produce different config hashes

### 5b. Config diff tool
- `cargo run --bin config_diff -- out/configs/a.json out/configs/b.json`
- Shows which parameters changed and in what direction

---

## Phase 6: Dead Code Resolution (2-3 commits)

### 6a. Composable strategies: benchmark or delete
- Wire `strategies.rs` into a backtest binary
- Run on all 5 regimes, compare to SimpleMomentum baseline
- Decision gate: keep if any beat baseline in any regime; delete otherwise

### 6b. Engine/reducer: scope or archive
- Either wire engine_backtest to run on real CSV data
- Or move engine/ to an `archive/` directory with a README explaining why
- No more pretending both architectures are production

### 6c. indicators.rs + signals.rs resolution
- Either wire into the SimpleMomentum path (replacing inline z-score computation)
- Or document them as reference implementations and mark aspirational

---

## Phase 7: Walk-Forward Validation (3 commits)

### 7a. Train/test split framework
- Split each regime dataset 70/30
- Backtest on train, validate on test
- Report overfit ratio: train_pnl / test_pnl

### 7b. Walk-forward harness
- Rolling window: train on N candles, test on next M
- Repeat across full dataset
- Aggregate: mean return, variance, worst window

### 7c. Multiple testing correction
- 12 strategies x 5 regimes = 60 comparisons
- Apply Bonferroni or Holm-Bonferroni correction
- Flag any "significant" result that doesn't survive correction

---

## Phase 8: Automated Hypothesis Loop (2-3 commits)

### 8a. Backtest → ledger updater
- `cargo run --bin update_ledger -- out/reports/latest.json`
- Parses structured BacktestResult, computes Bayesian updates
- Writes updated hypothesis_ledger.edn

### 8b. Regime detection
- Classify new data into known regime categories
- Auto-select comparison baseline from historical results
- Report: "This looks like Regime X; in that regime, we expect Y"

### 8c. Change detection report
- Diff current ledger against previous version
- Report: which hypotheses strengthened, weakened, or flipped
- Highlight: any invariant that broke

---

## Phase 9: Publication Quality (2 commits)

### 9a. GitHub Pages with live data
- Pages workflow reads hypothesis_ledger.edn at build time
- Renders current truth values, evidence summary, regime comparison
- No hardcoded HTML — generated from data

### 9b. Report format standardization
- Pipeline produces JSON + Markdown report
- JSON for programmatic consumption
- Markdown for human reading / GitHub rendering

---

## Release Criteria for v0.3.0

1. `cargo test` — 0 failures, 350+ tests
2. `scripts/smoke.sh` — all smoke tests green
3. Dead code resolved one way or the other (strategies.rs, engine/)
4. Walk-forward validation on at least one regime
5. Multiple testing correction applied to all "significant" claims
6. Pipeline: fetch → validate → backtest → update ledger → report
7. Config snapshot on every run
8. At least one strategy with positive net PnL surviving walk-forward,
   OR honest documentation of why not (and what would need to change)

---

## Non-Goals for v0.3.0

- Live trading (requires exchange integration testing)
- Multi-asset (requires data infrastructure)
- ML/optimization (requires walk-forward first to avoid overfitting)
- Real-time dashboards (batch analysis is sufficient)
