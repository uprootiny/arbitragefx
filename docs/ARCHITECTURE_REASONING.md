# Architectural Reasoning: Pivots, Dead Ends, and Viable Paths

**Date:** 2026-02-20
**Purpose:** Reconcile the project's evolutionary path — what was built, what was abandoned, what has unrealized value, and what capability profile is feasible.

---

## The Story of Two Systems

ArbitrageFX grew by accretion, not by design. The result is two parallel systems sharing a crate boundary:

### System A: The Imperative Loop (production, tested)

```
state.rs (2160 lines)
  └── IndicatorState (inline EMA, vol, z-score computation)
  └── MarketState (candle buffer per symbol)
  └── StrategyInstance (wraps SimpleMomentum)
  └── Config (55 env vars)

backtest.rs (842 lines)
  └── run_backtest() — CSV → candle → indicator → strategy → risk → fill → metrics
  └── Execution sim: slippage, fees, latency, partial fills

strategy.rs (655 lines)
  └── SimpleMomentum (13-branch decision tree)
  └── 12 parameter variants (churn-0 through churn-11)
```

This is what runs. 281 tests validate it. Walk-forward analysis uses it. The hypothesis ledger is grounded in it. Its indicators are inline (not modular), its config is env vars (not files), and its strategy is monolithic (not composable). But it works.

### System B: The Modular Library (aspirational, untested)

```
indicators.rs (758 lines)
  └── Ema, Sma, Stdev, Rsi, Atr, Macd, Momentum
  └── Clean API: new(period) → update(price) → get()

strategies.rs (archived)
  └── 7 composable strategy types implementing Strategy trait
  └── Used modular signals and filters

signals.rs (archived)
  └── 16 signal functions: momentum, mean_reversion, trend, breakout...
  └── Consumed by strategies.rs (also archived)

filters.rs (archived)
  └── Volatility, trend, position limit filters
```

This was the "right" architecture. Modular indicators, composable signals, pluggable strategies. But it was never wired to the backtest loop. The backtest uses `IndicatorState` from `state.rs`, which reimplements EMA and z-scores inline.

### Why This Happened

1. **Velocity over structure**: The backtest loop needed to ship. Inline indicator computation was faster to write than wiring modular indicators through an interface boundary.

2. **The composable dream was too early**: `strategies.rs` required a signal/filter/sizing pipeline that was designed before the system had real data. When real data arrived, the pipeline's abstractions didn't match what SimpleMomentum actually needed (z-score blends, funding carry, liquidation cascades).

3. **Testing locked in the inline path**: Once 100+ tests validated the inline indicators, refactoring carried real regression risk.

---

## The Dormant Modules: What They Represent

Each dormant module encodes a capability that the system lacks but could benefit from:

### indicators.rs — Composable Indicator Library

**What it has:** Clean, tested implementations of Ema, Sma, Stdev, Rsi, Atr, Macd, Momentum with stateful rolling window semantics.

**What state.rs reimplements:** EMA (fast/slow), Welford online variance (price + vol + volume), VWAP, momentum, z-scores. All inline in `IndicatorState`.

**The gap:** `state.rs::IndicatorState` hardcodes exactly the indicators that SimpleMomentum needs. If we ever want RSI-based strategies, ATR-based stops, or Bollinger Band breakouts, we'd need to either:
- (a) Add more inline fields to IndicatorState (bad: grows the monolith)
- (b) Use the modular `indicators.rs` library (good: composable)

**Viable path:** Don't replace IndicatorState. Instead, make `indicators.rs` the canonical library and have IndicatorState *delegate* to it internally. The public API stays the same. New strategies can use `indicators.rs` directly.

**Effort:** ~50 lines of delegation glue. Zero API changes. Zero test breakage.

### drift_tracker.rs — Distribution Shift Detection

**What it has:** DriftSeverity enum (None/Low/Moderate/Severe/Critical), position multiplier mapping, feature-level drift reports, multi-feature aggregation.

**What it represents:** The capability to detect when market conditions have shifted away from what strategies were trained on. This is directly relevant to H002 (friction dominates) and H007 (no consistent alpha) — if the system could detect regime shifts in real-time, it could adapt position sizing.

**Current integration:** Only used by `main.rs` (live loop). Not used by backtest.

**Viable path:** Wire drift detection into the backtest loop as a *reporter* (not a filter). For each candle window, compute drift scores and include them in BacktestResult. This would let us answer: "Did strategies perform differently when drift was high?"

**Effort:** ~30 lines in backtest.rs to compute drift alongside indicators.

### storage.rs — SQLite Metric Persistence

**What it has:** Simple SQLite store for per-strategy equity/PnL/drawdown snapshots.

**What it represents:** The capability to persist backtest results for querying, comparison, and temporal analysis beyond JSONL append-only logs.

**Viable path:** Not high priority. The JSONL history and JSON artifacts serve the current need. SQLite would matter if we had hundreds of runs to query. Keep but don't prioritize.

### skeleton/ — Minimal Engine Test Harness

**What it has:** A stripped-down backtest loop with its own SimpleMomentum, PaperExec, WAL, Logger. Completely independent from the main backtest path.

**What it represents:** A parallel validation surface. If the main system computes X, skeleton should compute X independently.

**Viable path:** Keep as a "second opinion" implementation. Use it to verify that the main backtest loop is correct by running both and comparing results. This strengthens trap #5 (in-sample confusion) and trap #12 (backtest-to-live gap).

**Effort:** ~20 lines to add a comparison harness.

### fault/ — Chaos Injection

**What it has:** `FaultProfile` struct with `should_fault()` method.

**What it represents:** The capability to inject failures (network errors, partial fills, stale data) into the execution path for resilience testing.

**Viable path:** Would be valuable for stress-testing the risk guards. Currently untested whether the system degrades gracefully under fault injection. Low priority but real value for guarantee surface expansion.

---

## Capability Profile: What's Feasible

Based on what exists, here's the capability profile the system can realistically support:

### Tier 1: Already Working (just needs visibility)
| Capability | Module | Status |
|-----------|--------|--------|
| Momentum backtesting | backtest.rs + state.rs | Production, 281 tests |
| Walk-forward validation | walk_forward.rs | Production, Bonferroni correction |
| Regime classification | regime.rs + narrative_detector.rs | Production, 4 regimes |
| Hypothesis tracking | update_ledger.rs | Production, Bayesian updates |
| Observation dashboard | workbench.rs | Production, self-contained HTML |
| API server | epistemic_server.rs | Production, 4 endpoints |
| Data validation | data/mod.rs | Production, schema + SHA256 |

### Tier 2: Feasible with Integration (~50-100 lines each)
| Capability | Dormant Module | Integration Path |
|-----------|---------------|-----------------|
| Composable indicators | indicators.rs | Delegate from IndicatorState |
| Drift-aware backtesting | drift_tracker.rs | Add drift reporter to backtest loop |
| ATR-based stops | indicators.rs::Atr | New strategy variant using Atr for dynamic stops |
| RSI signal | indicators.rs::Rsi | New strategy variant using RSI for overbought/oversold |
| Skeleton cross-validation | skeleton/ | Comparison harness for trap #5 |

### Tier 3: Requires New Code (~200-500 lines each)
| Capability | Approach |
|-----------|---------|
| CLJS observation layer | shadow-cljs + Reagent frontend |
| Session templates | New binary + JSON schema |
| Reconciliation workflow | New binary reading trap/integrity data |
| Ontology graph | Force-directed graph from relationships |
| Config file (TOML) | Config parser + snapshot mechanism |

### Tier 4: Requires External Data (blocked)
| Capability | Blocker |
|-----------|---------|
| Calibrated slippage model | Need real fill data from live trading |
| Live trading loop | Need API keys, risk review, paper trading validation |
| Multi-asset correlation | Need ETH/SOL data with synchronized timestamps |

---

## Reconciling the Pivots

### Pivot 1: Composable → Monolithic Strategy

**What happened:** Started with 7 composable strategy types (strategies.rs), pivoted to SimpleMomentum with 12 parameter variants.

**Why it was right:** SimpleMomentum with churn variants is *searchable* — you can sweep 12 configurations and compare. Composable strategies require exponentially more combinations.

**What was lost:** The ability to mix signal types (momentum + mean_reversion + trend) in a single strategy. This matters for Tier 2.

**Resolution:** Keep SimpleMomentum as the primary strategy. Add new strategy types (RsiMeanRevert, AtrBreakout) that use `indicators.rs` directly. Don't try to compose them — run them independently and compare via walk-forward.

### Pivot 2: Event-Sourced Engine → Imperative Loop

**What happened:** Built engine/ directory with event bus, reducer, pure state transitions. Never wired to data.

**Why it was right:** The imperative loop is easier to test, debug, and profile. The reducer architecture adds complexity without enabling any feature the loop can't do.

**What was lost:** Replay and branching semantics (you can't easily "what-if" a different strategy decision mid-run).

**Resolution:** Archive engine/ entirely. The JSONL logging provides sufficient replay capability. If we ever need branching, we can fork the state at checkpoints within the imperative loop.

### Pivot 3: Indicator Library → Inline Computation

**What happened:** Built indicators.rs with clean modular types. Then built IndicatorState inline in state.rs that reimplements the same math.

**Why it was wrong:** This is the one pivot that cost us. The inline code works but prevents reuse. Every new indicator type requires growing IndicatorState.

**Resolution:** Delegate. Make IndicatorState use Ema, Sma from indicators.rs internally. Keep the public API unchanged. This is the single highest-ROI refactor.

### Pivot 4: Live Trading → Research Workbench

**What happened:** Started building toward live trading (exchange adapters, WAL, reconciliation). Discovered through hypothesis testing that no strategy generates consistent alpha.

**Why it was right:** The finding is the product. The system's honest self-assessment (H007: no consistent alpha) is more valuable than a live trading system running unprofitable strategies.

**What was lost:** The live infrastructure (exchange/, feed/, adapter/, reconcile/, reliability/) is complete but unused.

**Resolution:** Keep as infrastructure for future use. The live loop becomes relevant again only if we discover a strategy that survives walk-forward validation. Until then, the research workbench is the correct focus.

---

## Proposed Integration Plan

### Phase A: Indicator Delegation (Tier 2, highest ROI)

Wire `indicators.rs::Ema` into `state.rs::IndicatorState`:

```rust
// state.rs - change from:
struct IndicatorState {
    ema_fast: f64,
    ema_slow: f64,
    ...
}
// to:
struct IndicatorState {
    ema_fast: crate::indicators::Ema,
    ema_slow: crate::indicators::Ema,
    ...
}
```

Same behavior, same tests pass. But now `indicators.rs` is proven live code.

### Phase B: Drift Reporter in Backtest

Add drift computation alongside indicators in the backtest loop. Don't filter on it — just report it. This gives us data to test whether drift severity correlates with strategy performance.

### Phase C: New Strategy Types

Add RsiMeanRevert and AtrBreakout strategies that use `indicators.rs` types directly. Run them through the same walk-forward validation as SimpleMomentum. This tests whether different indicator types improve survival rates.

### Phase D: Cross-Validation Harness

Use skeleton/ as a second implementation. Run both systems on the same data and assert identical results. This provides an independent check that strengthens the guarantee surface.

---

## What To Delete

| Module | Lines | Action | Reason |
|--------|-------|--------|--------|
| `fault/inject.rs` | 22 | **Delete** | Too minimal to be useful. If we need chaos injection, we'll build it properly. |
| Engine loop (archived) | ~4,000 | **Keep archived** | Already moved to archive/. Leave it there. |
| strategies.rs (archived) | ~800 | **Keep archived** | May inspire future strategy types. |
| signals.rs (archived) | ~460 | **Keep archived** | Some signal functions may be useful for new strategies. |

Only `fault/` is worth actually deleting — it's 22 lines that do nothing.

---

## Summary: The System's Identity

ArbitrageFX is not a trading system. It is a **hypothesis-driven research workbench** that uses backtesting as its primary observation method.

Its core loop (CSV → backtest → walk-forward → hypothesis update → dashboard) is complete and validated. Its dormant modules represent capabilities that were built too early — before the system had the data and evidence to know which capabilities it actually needed.

The architectural lesson: **build capabilities when you have evidence that you need them, not when you imagine you might.** The indicators library was built because "a trading system needs indicators." The hypothesis ledger was built because the evidence showed we needed it. Guess which one is in production.

The path forward is integration, not deletion. Wire the dormant modules into the live path where evidence supports the need. Archive what remains unused. And keep listening to what the hypotheses tell us.

---

*"The system that honestly reports 'I found nothing' has found something: it has found its own limits."*
