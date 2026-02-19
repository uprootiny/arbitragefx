# ArbitrageFX — Design Document v3

**Date:** 2026-02-18, **Updated:** 2026-02-19
**Status:** Living document, updated from real backtest evidence

## 1. What This System Is

A Rust-based cryptocurrency **backtesting, trading reconnaissance, strategy
evaluation, and hypothesis tracking** workbench. It ingests OHLCV candle data,
runs momentum/carry/event-driven strategies through a friction-aware execution
simulator, and reports PnL with honest accounting of fees, slippage, latency,
and partial fills.

It maintains a hypothesis ledger with Bayesian truth values updated from
multi-regime backtest evidence. The epistemic server exposes this state as
JSON for dashboard visualization.

## 2. System Capabilities

| Capability | Status | Evidence |
|-----------|--------|----------|
| Backtest on real OHLCV data | Production | 5 regimes, 353 tests |
| Friction-aware execution sim | Production | 4 exec modes, deterministic |
| 12-variant parameter sweep | Production | churn set in state.rs |
| Hypothesis tracking (Bayesian) | Production | 9 hypotheses, v1.1.0 |
| Data validation + provenance | Production | SHA256, schema, gap detection |
| Smoke test suite | Production | 13 Rust + 8 shell tests |
| Structured results (BacktestResult) | Production | per-strategy breakdown |
| Min hold period (anti-overtrading) | Production | min_hold_candles param |
| Pipeline: fetch → validate → backtest → report | Production | scripts/pipeline.sh |
| Epistemic server (real data) | Production | reads hypothesis_ledger.edn |
| GitHub Pages docs | Deployed | pages.yml workflow |
| Walk-forward validation | Not yet | Phase 7 |
| Automated hypothesis updates | Not yet | Phase 8 |
| Live/paper trading | Not yet | no timeline |

## 3. Architecture: Two Parallel Paths

### Legacy Loop (production-tested)
```
CSV → parse_csv_line → MarketState.on_candle → IndicatorState.update
    → SimpleMomentum.update → RiskEngine.apply → PendingOrder queue
    → latency_delay → slippage/fill sim → equity/metrics tracking
    → BacktestResult { total_pnl, max_drawdown, buy_hold_pnl, strategies[] }
```
- Entry: `src/bin/backtest.rs` → `backtest::run_backtest()` / `run_backtest_full()`
- State: `MarketState` + `StrategyInstance` + `RiskEngine` (all in `state.rs`)
- Strategies: `SimpleMomentum` (12 churn variants), `CarryOpportunistic` (3 variants)
- This is what actually runs. 353 tests pass against it.

### Engine Loop (aspirational, not wired)
```
Event → EventBus → Reducer(State, Event) → (State', Commands)
```
- Better architecture but no CSV ingestion, no real strategy implementations.
- Decision pending: scope to production or archive.

## 4. Strategy Design Space

### SimpleMomentum Decision Tree
```
update(market, state) →
  1. start_delay check         → Hold
  2. vol_pause check           → Hold
  3. trend filter (EMA cross)
  4. score = 1.0*z_mom + 0.3*z_vol + 0.5*z_vol_spike + stretch_contrib
  5. edge_hurdle check         → Hold if expected_edge < hurdle
  6. funding carry trade       → Buy/Sell (funding imbalance)
  7. liquidation cascade       → Buy/Sell (with impulse)
  8. depeg snapback            → Buy/Sell (fade depeg)
  9. position exit:
     a. stop_loss (always fires, ignores min_hold)
     b. take_profit (respects min_hold)
     c. time_stop (respects min_hold)
     d. exit_threshold (respects min_hold)
 10. vol regime switch:
     a. low vol → follow momentum
     b. high vol → trend-aligned mean reversion
 11. score-based entry with trend confirmation
 12. strong trend override
 13. default: Hold
```

All 13 branches are tested.

### Parameter Sweep (churn set)
| Variant | entry_th | edge_scale | TP | SL |
|---------|----------|------------|-----|-----|
| churn-0..2 | 1.2 | 0.0025 | 0.006/0.01/0.015 | 0.004/0.006/0.008 |
| churn-3..5 | 1.5 | 0.002 | same | same |
| churn-6..8 | 0.8 | 0.003 | same | same |
| churn-9..11 | 1.0 | 0.0015 | same | same |

## 5. Execution Model

| Mode | Slippage | Fees | Latency | Fill Ratio | Use |
|------|----------|------|---------|------------|-----|
| Instant | 0 | 0 | 0 | 100% | Unit tests |
| Market | k-scaled | taker | bounded jitter | 100% | Default backtest |
| Limit | low | maker | higher jitter | 80% | Passive strategies |
| Realistic | vol-scaled | blended | full model | 60% | Honest assessment |

Deterministic: latency uses xorshift from (submit_ts, strategy_idx), not RNG.

## 6. Data

### Real Datasets (Binance public API)
| File | Regime | Candles | Move | Period |
|------|--------|---------|------|--------|
| btc_real_1h.csv | Strong Bear | 1000 | -27% | Jan-Feb 2026 |
| btc_bull_1h.csv | Strong Bull | 2209 | +49% | Oct-Dec 2024 |
| btc_range_1h.csv | Ranging | 2209 | +1% | Jul-Sep 2024 |
| btc_bear2_1h.csv | Mild Bear | 1465 | -2% | Mar-May 2024 |
| btcusdt_1h_20260219.csv | Bear | 1000 | -24% | Feb 2026 |

All with validated schema (11 columns), SHA256 hashes, gap detection.

## 7. What We Know (from hypothesis_ledger.edn v1.1.0)

| ID | Hypothesis | (stv S C) | Assessment |
|----|-----------|-----------|------------|
| H001 | Momentum generates raw alpha | (0.42 0.72) | Improving, 3/5 datasets show some |
| H002 | Friction dominates alpha | (0.88 0.75) | Well-established |
| H003 | Position sizing limits DD <2% | (0.95 0.80) | Well-established |
| H004 | Trend filter improves quality | (0.70 0.65) | Mixed evidence |
| H005 | Vol regime switching helps | (0.55 0.58) | Weak evidence |
| H006 | Regime-specific strategies needed | (0.75 0.70) | Supported |
| H007 | No strategy consistently beats no-trade | (0.82 0.75) | Well-established |
| H008 | Trade frequency is primary friction driver | (0.90 0.70) | Strong |
| H009 | System's edge is capital preservation | (0.88 0.78) | Well-established |

The system preserves capital well but does not generate consistent returns.
This is itself a finding worth reporting honestly.

## 8. Epistemic Server

`src/epistemic.rs` → `EpistemicState::from_system()`:
- Parses hypothesis_ledger.edn for real hypothesis data
- Scans data/ for CSV file count
- Reports actual signals, filters, dataflows from production code
- Honestly declares assumptions (uncalibrated slippage, no live validation)
- Served as JSON at `/api/state` by `src/bin/epistemic_server.rs`

## 9. Config

55 parameters (54 + min_hold_candles), all from environment variables.
No config file, no CLI args. Good for containers, bad for reproducible
experiments. Config snapshotting is planned for v0.3.0 Phase 5.

## 10. Module Map

25 top-level modules, ~15,700 lines of library code, ~4,000 lines of binaries.
23 binaries. 353 tests (all pass).

**Core path:** `strategy.rs` → `state.rs` → `backtest.rs` → `risk.rs`
**Data path:** `data/mod.rs` → `src/bin/dataset_manifest.rs` → `src/bin/coherence_check.rs`
**Engine path:** `engine/events.rs` → `engine/state.rs` → `engine/reducer.rs` (aspirational)
**Support:** `indicators.rs`, `signals.rs`, `filters.rs`, `sizing.rs`, `features.rs`
**Infra:** `exchange/`, `feed/`, `adapter/`, `reliability/`, `verify/`, `logging.rs`
**Epistemic:** `epistemic.rs` → `hypothesis.rs` → `hypothesis_ledger.edn`
