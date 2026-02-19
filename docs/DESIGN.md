# ArbitrageFX — Design Document v2

**Date:** 2026-02-18
**Status:** Living document, updated from real backtest evidence

## 1. What This System Is

A Rust-based cryptocurrency backtesting and (aspirationally) live-trading workbench.
It ingests OHLCV candle data, runs momentum/carry/event-driven strategies through
a friction-aware execution simulator, and reports PnL with honest accounting of
fees, slippage, latency, and partial fills.

## 2. Architecture: Two Parallel Paths

The codebase contains **two complete architectures** that do not share a runtime path:

### Legacy Loop (production-tested)
```
CSV → parse_csv_line → MarketState.on_candle → StrategyInstance.update
    → RiskEngine.apply → PendingOrder queue → slippage/fill sim → metrics
```
- Entry: `src/bin/backtest.rs` → `backtest::run_backtest()`
- State: `MarketState` + `StrategyInstance` + `RiskEngine` (all in `state.rs`)
- Strategies: `SimpleMomentum` (12 churn variants), `CarryOpportunistic` (3 variants)
- This is what actually runs. 324 tests pass against it.

### Engine Loop (aspirational)
```
Event → EventBus → Reducer(State, Event) → (State', Commands)
```
- Entry: `src/bin/engine_backtest.rs`, `src/bin/engine_loop.rs`
- State: `EngineState` (deterministically hashable)
- Pure reducer: `engine/reducer.rs` (834 lines, 10 tests)
- Ethics guards: `engine/ethics.rs` (three poisons)
- Narrative regimes: `engine/narrative_detector.rs`
- This is the better architecture. It is not wired to production data paths.

### The Gap
The engine loop has no CSV ingestion, no real strategy implementations plugged in,
and no connection to the hypothesis ledger. The legacy loop works but has strategies
baked into `state.rs` (1368 lines of mixed concerns).

## 3. Strategy Design Space

### What Exists (Legacy — actually runs)

| Factory | Count | Type | Parameterization |
|---------|-------|------|------------------|
| `build_churn_set` | 12 | `SimpleMomentum` | 4x3 grid over (entry_th, edge_scale, TP, SL) |
| `build_carry_event_set` | 3 | `CarryOpportunistic` | Staggered start delays |
| `build_default_set` | 3 | `SimpleMomentum` | Staggered start delays |

### What Exists (Composable — never instantiated)
Seven strategy types in `strategies.rs` (808 lines): Momentum, MeanReversion,
FundingCarry, VolatilityBreakout, EventDriven, MultiFactor, Adaptive.
These implement the `Strategy` trait but are never used by any binary.

## 4. Execution Model

Four modes with increasing realism:

| Mode | Slippage | Fees | Latency | Fill Ratio | Use |
|------|----------|------|---------|------------|-----|
| Instant | 0 | 0 | 0 | 100% | Unit tests |
| Market | k-scaled | taker | bounded jitter | 100% | Default backtest |
| Limit | low | maker | higher jitter | 80% | Passive strategies |
| Realistic | vol-scaled | blended | full model | 60% | Honest assessment |

Deterministic: latency uses xorshift from (submit_ts, strategy_idx), not RNG.

## 5. Risk Engine

Applied as a guard on every proposed action. In order:
1. Kill file check (`/tmp/STOP`)
2. Loss cooldown (600s default)
3. Daily trade limit (20/day)
4. Daily loss limit (2% equity, including unrealized MTM)
5. Exposure limit (5% of equity)

## 6. Data

### Real Datasets (from Binance public API)
| File | Regime | Candles | Move | Period |
|------|--------|---------|------|--------|
| btc_real_1h.csv | Strong Bear | 1000 | -27% | Jan-Feb 2026 |
| btc_bull_1h.csv | Strong Bull | 2209 | +49% | Oct-Dec 2024 |
| btc_range_1h.csv | Ranging | 2209 | +1% | Jul-Sep 2024 |
| btc_bear2_1h.csv | Mild Bear | 1465 | -2% | Mar-May 2024 |

## 7. What We Know (from hypothesis_ledger.edn)

| ID | Hypothesis | Strength | Confidence |
|----|-----------|----------|------------|
| H002 | Friction dominates alpha in all regimes | 0.88 | 0.75 |
| H003 | Position sizing limits drawdown to <2% | 0.95 | 0.80 |
| H007 | No strategy consistently beats no-trade | 0.82 | 0.75 |
| H008 | Trade frequency is the primary friction driver | 0.90 | 0.70 |

The system preserves capital well but does not generate consistent returns.

## 8. Config

54 parameters, all from environment variables. No config file, no CLI args.
Good for containers, bad for reproducible experiments (no config snapshotting).

## 9. Module Map

25 top-level modules, ~15,700 lines of library code, ~4,000 lines of binaries.
23 binaries. 258 tests (all pass).

Core path: `strategy.rs` -> `state.rs` -> `backtest.rs` -> `risk.rs`
Engine path: `engine/events.rs` -> `engine/state.rs` -> `engine/reducer.rs`
Support: `indicators.rs`, `signals.rs`, `filters.rs`, `sizing.rs`, `features.rs`
Infra: `exchange/`, `feed/`, `adapter/`, `reliability/`, `verify/`, `logging.rs`
