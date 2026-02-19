# ArbitrageFX Architecture

**Updated:** 2026-02-19 | **LOC:** ~15,700 (lib) + ~4,000 (bins) | **Tests:** 333

## Purpose

A cryptocurrency backtesting workbench that runs momentum and carry strategies
against real OHLCV data with honest friction accounting (fees, slippage, latency,
partial fills). Tracks hypotheses about strategy performance using Bayesian truth
values and produces structured evidence.

## System Diagram

```
                    ┌─────────────────────────────────────────────┐
                    │              Data Layer                      │
                    │                                             │
 Binance API ──────▶ fetch_data.sh ──▶ data/*.csv ──▶ data::analyze_csv()
                    │                  (SHA256 hashed, schema-validated)
                    └────────────────────────┬────────────────────┘
                                             │
                    ┌────────────────────────▼────────────────────┐
                    │           Backtest Engine                    │
                    │                                             │
                    │  parse_csv_line() ──▶ MarketState.on_candle │
                    │       │                                     │
                    │       ▼                                     │
                    │  StrategyInstance.update()                  │
                    │  (SimpleMomentum × 12 churn variants)      │
                    │       │                                     │
                    │       ▼                                     │
                    │  RiskEngine.apply_with_price()              │
                    │  (kill switch, cooldown, exposure, DD)      │
                    │       │                                     │
                    │       ▼                                     │
                    │  PendingOrder queue                         │
                    │  → latency_delay (xorshift deterministic)   │
                    │  → slippage_price (vol-scaled)              │
                    │  → partial fills (max_fill_ratio)           │
                    │  → fee accounting                           │
                    │       │                                     │
                    │       ▼                                     │
                    │  BacktestResult { strategies[], buy_hold }  │
                    └────────────────────────┬────────────────────┘
                                             │
                    ┌────────────────────────▼────────────────────┐
                    │          Evidence Layer                      │
                    │                                             │
                    │  hypothesis_ledger.edn                      │
                    │  ├── H001-H009: Bayesian truth values       │
                    │  ├── invariants I001-I004                   │
                    │  └── uncertainty_map                        │
                    │                                             │
                    │  pipeline.sh: fetch → validate → backtest   │
                    │               → report                      │
                    └────────────────────────┬────────────────────┘
                                             │
                    ┌────────────────────────▼────────────────────┐
                    │          Validation Layer                    │
                    │                                             │
                    │  333 tests (cargo test)                     │
                    │  ├── 164 lib unit tests                     │
                    │  ├── 128 bin unit tests                     │
                    │  ├── 26 backtest_validation integration     │
                    │  ├── 13 smoke tests (real data)             │
                    │  └── 3 drift integration tests              │
                    │                                             │
                    │  smoke.sh: 8 shell-level checks             │
                    │  CI: fmt + clippy + test + smoke + coherence│
                    └─────────────────────────────────────────────┘
```

## Module Map

### Core Path (what actually runs backtests)

| Module | Lines | Role |
|--------|-------|------|
| `strategy.rs` | 413 | Core types: Candle, MarketView, Action, Strategy trait |
| `state.rs` | 1368 | Config, MarketState, StrategyInstance, SimpleMomentum, CarryOpportunistic |
| `backtest.rs` | 620 | run_backtest(), ExecConfig, slippage/fill/latency models |
| `risk.rs` | 442 | RiskEngine: Kelly, exposure, cooldown, daily loss guards |
| `features.rs` | 137 | FeaturePipeline: rolling stats for funding/OI/vol |
| `events.rs` | 92 | Phase-1 event detection (funding, liquidation, depeg) |
| `metrics.rs` | 43 | MetricsEngine: equity tracking, drawdown |
| `data/` | 211 | CSV schema validation, gap detection, SHA256 hashing |

### Engine Path (event-sourced, not yet production)

| Module | Lines | Role |
|--------|-------|------|
| `engine/events.rs` | 277 | Typed events: Market/Exec/Sys for deterministic replay |
| `engine/state.rs` | 540 | EngineState: hashable, deterministic |
| `engine/reducer.rs` | 834 | Pure (State, Event) → (State', Commands) |
| `engine/ethics.rs` | 265 | Three poisons guard (greed/aversion/delusion) |
| `engine/narrative_detector.rs` | 440 | Regime classification with position multipliers |
| `engine/policy.rs` | 632 | Risk policy with drift awareness |
| `engine/drift_tracker.rs` | 547 | Distribution shift detection |

### Signal/Strategy Library (composable building blocks)

| Module | Lines | Role |
|--------|-------|------|
| `signals.rs` | 516 | 16 pure signal functions |
| `strategies.rs` | 808 | 7 composable Strategy impls |
| `indicators.rs` | 718 | EMA, RSI, MACD, Bollinger, ATR, Stochastic, patterns |
| `filters.rs` | 413 | Pre-trade filter chain |
| `sizing.rs` | 371 | Kelly, fixed fractional, vol-adjusted, ATR sizing |

### Infrastructure

| Module | Lines | Role |
|--------|-------|------|
| `exchange/` | 671 | Binance/Kraken REST clients, signing, retry |
| `feed/` | 795 | WebSocket feeds, aux data fetcher |
| `reliability/` | 699 | WAL, circuit breaker |
| `logging.rs` | 789 | Structured JSONL logging |
| `verify/` | 167 | Portfolio invariants, order state machine |
| `hypothesis.rs` | 828 | Hypothesis ledger Rust types |
| `epistemic.rs` | 369 | Epistemic state for dashboard |

## Execution Model

Four fidelity levels, all deterministic:

```
Instant:   fill_price = close, fee = 0, latency = 0
Market:    fill_price = close * (1 + slip), fee = taker, latency = [2,8]ms
Limit:     fill_price = close * (1 + slip), fee = maker, fill_prob < 1.0
Realistic: fill_price = vol-scaled, fee = blended, adverse_selection = 0.3
```

Latency uses xorshift from `(submit_ts, strategy_idx)` — no RNG, fully replayable.

## Risk Guards (applied to every action, in order)

1. Kill file (`/tmp/STOP`) → halt all trading
2. Loss cooldown (600s after loss) → close-only
3. Daily trade limit (20/day) → close-only
4. Daily loss limit (2% equity incl. unrealized) → force close
5. Exposure limit (5% equity) → block new positions

## Strategy Space

**Production (build_churn_set):** 12 variants of SimpleMomentum sweeping a
4×3 parameter grid over (entry_threshold, edge_scale, take_profit, stop_loss).
Composite z-score: `1.0 * z_momentum + 0.3 * z_vol + 0.5 * z_volume_spike + stretch`.

**Library (strategies.rs):** 7 composable types (Momentum, MeanReversion,
FundingCarry, VolBreakout, EventDriven, MultiFactor, Adaptive). Not wired
to the main backtest binary — available via strategy_sweep binary.

## Config

54 parameters from environment variables. Key parameters:

| Parameter | Default | Category |
|-----------|---------|----------|
| SYMBOL | BTCUSDT | Core |
| CANDLE_SECS | 300 | Core |
| MAX_POS_PCT | 5% | Risk |
| MAX_DAILY_LOSS_PCT | 2% | Risk |
| COOLDOWN_SECS | 600 | Risk |
| ENTRY_TH | 1.2 | Signal |
| TAKE_PROFIT | 0.6% | Exits |
| STOP_LOSS | 0.4% | Exits |
| FEE_RATE | 0.1% | Execution |
| SLIP_K | 0.08% | Execution |

## What We Know (from 60 real backtest runs)

| Finding | Confidence |
|---------|------------|
| Friction dominates alpha (~90% of cases) | 0.80 |
| Position sizing bounds drawdown to <2% (4/5 regimes) | 0.84 |
| Trade frequency is the primary friction driver | 0.76 |
| The system's edge is capital preservation, not alpha | 0.55 |
| No strategy consistently beats no-trade | 0.80 |

## Binaries

| Binary | Purpose |
|--------|---------|
| `backtest` | Core: CSV → backtest → results |
| `backtest_logged` | Backtest with structured JSONL output |
| `strategy_sweep` | Run composable strategies from strategies.rs |
| `research_lab` | Hypothesis-driven experiment runner |
| `coherence_check` | Validate hypothesis ledger against data |
| `dataset_manifest` | Generate SHA256 manifest for CSV |
| `epistemic_server` | HTTP API for dashboard |
| `engine_backtest` | Event-sourced backtest (experimental) |
| `diagnose` | Per-bar strategy decision trace |

## Test Strategy

- **Unit tests** (164): individual function correctness
- **Integration tests** (32): cross-module behavior
- **Smoke tests** (13): real data end-to-end validation
- **Backtest validation** (26): execution model invariants
- **Drift tests** (3): regime-change response
- **Shell smoke** (8): binary-level sanity checks
- **CI**: fmt → clippy → build → test → smoke → coherence
