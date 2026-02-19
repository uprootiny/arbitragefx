# ArbitrageFX — Critique: Wishful Thinking Inventory

**Date:** 2026-02-18
**Purpose:** Honest assessment of where the codebase claims more than it delivers.

---

## 1. Two Architectures, Zero Convergence

The engine/ directory (3,964 lines) implements a beautiful event-sourced reducer
architecture. The actual backtest runs through `state.rs` + `backtest.rs`, which
is a mutable-state imperative loop. These share no code paths.

**The wishful thinking:** "We have a pure functional event-sourced architecture."
**The reality:** We have a working imperative loop AND an untested functional
skeleton. The engine_backtest binary exists but is not used for any hypothesis
testing, sweep, or validation run.

**Cost:** Every improvement to the legacy loop (friction models, strategy variants)
must eventually be reimplemented in the engine, or the engine must be abandoned.
Neither has happened.

## 2. strategies.rs: 808 Lines of Dead Code

Seven composable strategy types (`MomentumStrategy`, `MeanReversionStrategy`,
`FundingCarryStrategy`, `VolatilityBreakoutStrategy`, `EventDrivenStrategy`,
`MultiFactorStrategy`, `AdaptiveStrategy`) implement the `Strategy` trait.

**None of them are instantiated by any binary.**

The actual backtests use `SimpleMomentum` and `CarryOpportunistic` from `state.rs`.
The composable strategies exist to prove the design is extensible. They prove
nothing about alpha.

**Cost:** 808 lines that create the impression of strategic diversity while
all real results come from one strategy with parameter sweeps.

## 3. signals.rs: A Library Nobody Calls

16 signal functions (momentum, mean_reversion, trend_strength, vwap_deviation,
vol_breakout, climax_reversal, funding_carry, etc.)

These are consumed only by the dead `strategies.rs`. The live `SimpleMomentum`
computes its own z-score blend inline.

## 4. The Epistemic Server Lies

`src/epistemic.rs` -> `EpistemicState::from_system()` returns **hardcoded literals**.
It claims to scan the codebase and report verification strata. It does not.
The numbers it serves are fiction presented as introspection.

The dashboard at port 42280 displays these lies as if they were computed.

## 5. 23 Binaries, 1 Workflow

The codebase has 23 binaries. The actual workflow is:
```
cargo run --bin backtest -- data/btc_real_1h.csv
```
That's it. The other 22 binaries are:
- 5 that don't compile without specific data/config
- 8 that duplicate functionality of others
- 4 stress/fuzz tools that have never caught a bug
- 3 that analyze logs that are never generated in normal operation
- 2 skeleton demos

**Cost:** Compilation time. Every `cargo test` builds all 23 binaries.

## 6. "Hypothesis-Driven Research" Without the Loop

`hypothesis.rs` (828 lines) and `src/bin/research_lab.rs` (679 lines) implement
a structured hypothesis tracking system. The hypothesis_ledger.edn was built
**manually** by running backtests and interpreting output.

There is no automated path from "run backtest" -> "update hypothesis truth values."

## 7. Config: 54 Environment Variables, Zero Snapshots

Every run's configuration is determined by environment variables. There is no
mechanism to:
- Record which config produced which results
- Diff configs between runs
- Restore a previous config
- Validate that a config is internally consistent

## 8. The "Realistic" Execution Mode Isn't

`ExecConfig::realistic()` uses:
- `slippage_k: 0.0005` — plausible but not calibrated to any real orderbook
- `adverse_selection: 0.3` — pulled from thin air
- `vol_slip_mult: 1.5` — same

None of these parameters are derived from observed fill data.

## 9. Data Provenance Is Partial

The four regime datasets have SHA256 hashes. Good. But:
- The older datasets (btc_5m_30d.csv, btc_1h_180d.csv) have unknown provenance
- No dataset records its fetch timestamp or API parameters
- The train/test splits were created by an unknown process
- `btc_sim.csv` is synthetic but not labeled as such

## 10. "Live Trading" Is Theoretical

The codebase has exchange traits, WebSocket feeds, WAL, reconciliation, kill switch,
circuit breaker. But:
- No evidence of a single live trade ever executed
- No paper trading results
- No deployment configuration, monitoring, or alerting

## 11. The Narrative Detector Has No Evidence

`NarrativeRegime` classifies markets as Grounded/Uncertain/NarrativeDriven/Reflexive
and multiplies position sizes by 1.0/0.7/0.3/0.0. The thresholds were chosen
by intuition. There is no backtest showing this improves returns.

## 12. Test Coverage Is Shallow Where It Matters

258 tests sounds substantial. But:
- `SimpleMomentum.update()` (the actual trading decision) has **zero direct tests**
- No test verifies that a strategy produces expected trades on known data
- The backtest_validation tests verify execution mechanics, not strategy correctness
- `state.rs` tests are mostly for RingBuffer and Config parsing

## 13. indicators.rs: Comprehensive but Disconnected

718 lines implementing EMA, SMA, RSI, MACD, Bollinger Bands, ATR, etc.
Used by: `filters.rs` (which is used by nobody in the backtest path).
The actual backtest uses `IndicatorState` from `state.rs`, which computes its
own indicators inline.

## 14. Documentation Debt

32 markdown files totaling ~3,000 lines. Many describe aspirations rather than
reality. `ARCHITECTURE.md` describes the engine architecture that isn't used.
`ROADMAP_BACKTEST.md` has 52 tickets; 24 are unimplemented.

---

## Summary: What Is Real vs What Is Wishful

| Real | Wishful |
|------|---------|
| CSV ingestion + backtest loop | Engine/reducer architecture |
| 12-strategy parameter sweep | Composable strategies (strategies.rs) |
| Friction accounting | Signal library (signals.rs) |
| Risk guards | Indicator library (indicators.rs) |
| Deterministic replay | Epistemic server data |
| 4 real-data regime backtests | Live trading |
| Hypothesis ledger (manual) | Automated hypothesis updates |
| | Config reproducibility |

The honest core is ~3,000 lines. The rest is scaffolding for futures that
haven't arrived.
