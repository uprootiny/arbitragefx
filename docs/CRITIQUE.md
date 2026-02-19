# ArbitrageFX — Critique: Wishful Thinking Inventory

**Date:** 2026-02-18, **Updated:** 2026-02-19
**Purpose:** Honest assessment of where the codebase claims more than it delivers.

---

## Status Key

- **FIXED** — addressed with evidence
- **PARTIAL** — progress made, gap remains
- **OPEN** — not yet addressed

---

## 1. Two Architectures, Zero Convergence — OPEN

The engine/ directory (3,964 lines) implements a beautiful event-sourced reducer
architecture. The actual backtest runs through `state.rs` + `backtest.rs`, which
is a mutable-state imperative loop. These share no code paths.

**The wishful thinking:** "We have a pure functional event-sourced architecture."
**The reality:** We have a working imperative loop AND an untested functional
skeleton. The engine_backtest binary exists but is not used for any hypothesis
testing, sweep, or validation run.

**Cost:** Every improvement to the legacy loop must eventually be reimplemented
in the engine, or the engine must be abandoned. Neither has happened.

**Plan:** v0.3.0 Phase 6b will scope or archive the engine.

## 2. strategies.rs: 808 Lines of Dead Code — OPEN

Seven composable strategy types implement the `Strategy` trait.
**None of them are instantiated by any binary.**

**Plan:** v0.3.0 Phase 6a will benchmark against SimpleMomentum or delete.

## 3. signals.rs: A Library Nobody Calls — OPEN

16 signal functions consumed only by the dead `strategies.rs`.
The live `SimpleMomentum` computes its own z-score blend inline.

**Plan:** Resolved together with strategies.rs in Phase 6.

## 4. The Epistemic Server Lies — FIXED

`EpistemicState::from_system()` previously returned **hardcoded literals**.

**Fix (commit 90903ea):** Now parses `hypothesis_ledger.edn` for real hypothesis
IDs, names, and Bayesian truth values. Scans `data/` for CSV count. Reports
actual signals, filters, and pipeline dataflows from the production code path.
Honestly declares uncalibrated slippage and absent live validation as assumptions.

## 5. 23 Binaries, 1 Workflow — PARTIAL

Added useful binaries: `coherence_check`, `dataset_manifest`.
Added pipeline script that orchestrates: fetch → validate → backtest → report.
Still have ~18 binaries of questionable utility.

**Plan:** Phase 6 will audit and prune.

## 6. "Hypothesis-Driven Research" Without the Loop — PARTIAL

The hypothesis_ledger.edn is now at v1.1.0 with 9 hypotheses, 5 datasets,
and Bayesian truth values updated from real backtest evidence.
But the update process is still manual.

**Plan:** v0.3.0 Phase 8a will automate: backtest → parse → Bayesian update → write.

## 7. Config: 54 Environment Variables, Zero Snapshots — OPEN

No config snapshotting. No way to reproduce a specific run.

**Plan:** v0.3.0 Phase 5.

## 8. The "Realistic" Execution Mode Isn't — OPEN

`ExecConfig::realistic()` parameters are plausible but uncalibrated.
Now honestly declared as an assumption in the epistemic server.

**Plan:** Requires real fill data to calibrate. Blocked until live/paper trading.

## 9. Data Provenance Is Partial — PARTIAL

Five regime datasets with SHA256 hashes and known provenance (Binance public API,
fetch timestamps recorded in hypothesis_ledger.edn). Headers standardized.
Older datasets (btc_5m_30d.csv, btc_1h_180d.csv) still have unknown provenance.

## 10. "Live Trading" Is Theoretical — OPEN

No evidence of a single live trade ever executed.
Honestly declared as an assumption (confidence: 0.0) in the epistemic server.

## 11. The Narrative Detector Has No Evidence — OPEN

`NarrativeRegime` thresholds were chosen by intuition. No backtest evidence.

## 12. Test Coverage Is Shallow Where It Matters — FIXED

**Fix (commits cef7b81, 5b959e8, 9d4799f):**
- 15 direct tests for SimpleMomentum.update() covering all decision branches:
  start delay, vol pause, funding carry, liquidation cascade, take profit,
  stop loss, time stop, score-based entry (buy/sell), edge hurdle, low-vol
  momentum follow, high-vol mean reversion, strong trend override,
  min_hold_candles (blocks TP, allows stop loss)
- 13 Rust smoke tests on real data (schema, regression, determinism, cross-regime)
- 353 total tests passing

## 13. indicators.rs: Comprehensive but Disconnected — OPEN

718 lines implementing EMA, SMA, RSI, MACD, Bollinger Bands, ATR, etc.
Used by `filters.rs` (which is used by nobody in the backtest path).
The actual backtest uses `IndicatorState` from `state.rs`.

**Plan:** Resolved with strategies.rs in Phase 6.

## 14. Documentation Debt — PARTIAL

**Fixed:**
- ARCHITECTURE.md rewritten with actual system diagram and module map
- README.md created with honest quick start and findings
- DESIGN.md updated to distinguish legacy vs engine architecture
- ROADMAP_NEXT_RELEASE.md updated with v0.2.0 completion status and v0.3.0 plan
- ROADMAP_BACKTEST.md updated with 30/52 ticket completion

**Remaining:** Some older docs still describe aspirations as reality.

---

## Summary: What Is Real vs What Is Wishful (updated)

| Real | Wishful | Status |
|------|---------|--------|
| CSV ingestion + backtest loop | Engine/reducer architecture | OPEN |
| 12-strategy parameter sweep | Composable strategies (strategies.rs) | OPEN |
| Friction accounting | Signal library (signals.rs) | OPEN |
| Risk guards | Indicator library (indicators.rs) | OPEN |
| Deterministic replay | ~~Epistemic server data~~ | **FIXED** |
| 5 real-data regime backtests | Live trading | OPEN |
| Hypothesis ledger (v1.1.0, 9 hypotheses) | Automated hypothesis updates | PARTIAL |
| 353 tests, 15 strategy decision tests | ~~Shallow test coverage~~ | **FIXED** |
| Smoke tests + CI pipeline | Config reproducibility | OPEN |
| Structured BacktestResult | Walk-forward validation | OPEN |
| min_hold_candles | Multiple testing correction | OPEN |

**Scorecard:** 3 fixed, 4 partial, 7 open. Net drift: toward structure.

The honest core is ~3,500 lines (up from ~3,000). The delta is real:
data validation, structured results, strategy tests, honest epistemic reporting.
