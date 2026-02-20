# mindful-engine (archived)

An event-driven trading engine with Buddhist-inspired ethical guards
and deterministic replay semantics.

## What's here

### Core engine
- **engine/reducer.rs** (834 lines) — Pure `(State, Event) → (State, Commands)`
  reducer. Deterministic state transitions. Mean-reversion signal logic.
- **engine/events.rs** (277 lines) — Event type definitions.
- **engine/state.rs** (540 lines) — Engine state with indicator tracking.
- **engine/bus.rs** (157 lines) — Ordered event bus.
- **engine/policy.rs** (632 lines) — Agent intent → risk gate → execution.
  Separates what the agent wants from what the system allows.

### Ethical framework
- **engine/eightfold_path.rs** (695 lines) — Maps the Noble Eightfold Path
  to system health checks. Right View = honest uncertainty, Right Intention =
  survivability over greed, Right Speech = truthful logging, etc.
- **engine/ethics.rs** (265 lines) — Three-poisons guards (greed, aversion,
  delusion) as runtime checks.
- **engine/backtest_ethics.rs** (549 lines) — Ethical constraints specific
  to backtesting (anti-overfitting, anti-cherry-picking).

### Research infrastructure
- **engine/narrative_detector.rs** (440 lines) — NOW WIRED INTO ACTIVE CODEBASE
  as `src/narrative_detector.rs`. Regime classification from funding/liquidation/
  volatility signals.
- **engine/backtest_traps.rs** (748 lines) — NOW WIRED INTO ACTIVE CODEBASE
  as `src/backtest_traps.rs`. 18-trap backtest integrity checklist.
- **engine/experiment_registry.rs** (463 lines) — Append-only trial logging
  with Bonferroni, Holm-Bonferroni, and FDR correction methods.
- **engine/drift_tracker.rs** (547 lines) — NOW STANDALONE at `src/drift_tracker.rs`.
  Distribution drift detection for live trading.
- **engine/logging.rs** (643 lines) — Structured engine logging.

### Binaries
- **engine_backtest.rs** — Backtest via the event-driven engine.
- **engine_loop.rs** — Live trading loop using the engine.
- **path_check.rs** — Eightfold path compliance checker.
- **trials.rs** — Experiment runner with multiple testing correction.
- **drift_integration.rs** — Integration test for drift detection.

## Why archived

The active backtest pipeline (backtest.rs) uses a simpler loop architecture
that processes candles directly. The event-driven engine was designed for
live trading with deterministic replay, but only `drift_tracker` made it
into the production live_loop. The three most valuable pieces have been
extracted back into the active codebase.

## Spin-off potential

This is a unique project: an event-driven trading engine with formalized
Buddhist cognitive hygiene as operational epistemology. Nothing else like
this exists in the Rust trading ecosystem. The eightfold_path + ethics +
narrative_detector + backtest_traps modules form a coherent "mindful trading"
framework that could be a standalone crate.

## Quality

All modules compile independently. Tests exist for reducer, ethics,
narrative_detector, backtest_traps, experiment_registry, drift_tracker.
The architecture is sound — pure reducer pattern with explicit state.
