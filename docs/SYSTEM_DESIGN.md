# ArbitrageFX — System Design (As-Is)

**Date:** 2026-02-20
**Git SHA:** a83abaf
**Tests:** 281 passing | **Integrity:** 9/18 traps guarded | **Hypotheses:** 9

---

## 1. Purpose

A Rust cryptocurrency backtesting workbench with honest friction accounting and hypothesis-driven strategy evaluation. The system ingests OHLCV candle data, runs momentum strategies through a friction-aware execution simulator, updates a Bayesian hypothesis ledger, and publishes observable results through a self-contained dashboard and JSON API.

**Key finding (from 60+ backtest runs):** The system preserves capital well but does not generate consistent returns. This is itself a finding worth reporting honestly.

---

## 2. System Diagram

```
                         ┌─────────────────────────────────────────┐
                         │           DATA INGESTION                │
                         │                                         │
   Binance API ──────────┤  fetch_data.sh → data/*.csv             │
   (1h candles)          │  validate: schema, gaps, SHA256         │
                         │  16 datasets, 4 regime classifications  │
                         └────────────┬────────────────────────────┘
                                      │
                         ┌────────────▼────────────────────────────┐
                         │          BACKTEST ENGINE                 │
                         │                                         │
                         │  CsvRow → MarketState.on_candle()       │
                         │       → IndicatorState.update()         │
                         │       → SimpleMomentum.update() x 12    │
                         │       → RiskEngine.apply_with_price()   │
                         │       → PendingOrder queue              │
                         │       → latency → slippage → fill       │
                         │       → MetricsEngine (equity, DD)      │
                         │                                         │
                         │  ExecMode: Instant|Market|Limit|Realist │
                         │  Deterministic: xorshift from (ts, idx) │
                         └────────────┬────────────────────────────┘
                                      │
              ┌───────────────────────┼───────────────────────┐
              │                       │                       │
  ┌───────────▼──────────┐ ┌─────────▼──────────┐ ┌─────────▼──────────┐
  │   WALK-FORWARD       │ │   REGIME CLASSIFY   │ │   BENCH PROFILER   │
  │                      │ │                      │ │                    │
  │  4 windows, 70/30    │ │  NarrativeDetector   │ │  Timing per dataset│
  │  Bonferroni correct  │ │  4 regimes:          │ │  Throughput (c/s)  │
  │  p-value per strat   │ │  Grounded/Uncertain  │ │  Peak RSS (KB)     │
  │  0/12 survive        │ │  NarrativeDriven     │ │  JSON to out/bench │
  └───────────┬──────────┘ │  Reflexive           │ └─────────┬──────────┘
              │            └──────────────────────┘           │
              │                                               │
  ┌───────────▼──────────────────────────────────────────────▼─┐
  │                    HYPOTHESIS LEDGER                        │
  │                                                            │
  │  hypothesis_ledger.edn — 9 hypotheses with Bayesian STVs  │
  │  update_ledger.rs — parse results → compute updates        │
  │  Append JSONL to out/ledger_history/updates.jsonl          │
  │                                                            │
  │  H001: Raw alpha           (stv 0.42 0.72) contested      │
  │  H002: Friction dominates  (stv 0.82 0.80) supported      │
  │  H003: DD bounded <2%     (stv 0.95 0.84) established     │
  │  H007: No consistent edge  (stv 0.78 0.80) supported      │
  │  H008: Frequency→friction  (stv 0.91 0.76) established    │
  │  H009: Edge = preservation (stv 0.85 0.55) established    │
  └──────────────────────────┬─────────────────────────────────┘
                             │
  ┌──────────────────────────▼─────────────────────────────────┐
  │                   OBSERVATION SURFACE                       │
  │                                                            │
  │  workbench.rs → docs/workbench.html (self-contained)       │
  │    8 overview cards (tests, datasets, throughput, etc.)     │
  │    Strategy x Regime heatmap                               │
  │    Walk-forward survival table                             │
  │    Hypothesis ledger with truth values                     │
  │    Evidence timeline with sparklines                       │
  │    Uncertainty map kanban (5 columns)                      │
  │    18-point trap checklist (guard status)                  │
  │    Regime leaderboard (ranked by equity PnL)               │
  │    Resource trend charts (throughput over time)             │
  │                                                            │
  │  epistemic_server.rs → http://localhost:51723              │
  │    GET /          → workbench dashboard                    │
  │    GET /api/health → enriched health JSON                  │
  │    GET /api/state  → full epistemic state                  │
  │    GET /api/summary → compact status                      │
  └────────────────────────────────────────────────────────────┘
```

---

## 3. Module Map

### Core Path (~5,000 lines)
| Module | Lines | Purpose |
|--------|-------|---------|
| `state.rs` | 2160 | Config (55 params), MarketState, Fill, StrategyInstance, PortfolioState |
| `backtest.rs` | 842 | `run_backtest()`, CSV parsing, execution simulation, fill accounting |
| `strategy.rs` | 655 | `Strategy` trait, `SimpleMomentum` (13-branch decision tree), `CarryOpportunistic` |
| `risk.rs` | 505 | Kelly criterion, expectancy, risk-of-ruin, position limits |
| `indicators.rs` | 758 | EMA, SMA, RSI, MACD, ATR, Momentum (rolling window implementations) |

### Validation Path (~1,500 lines)
| Module | Lines | Purpose |
|--------|-------|---------|
| `walk_forward.rs` | 331 | Train/test splits, Bonferroni correction, overfit ratio |
| `backtest_traps.rs` | 834 | 18 trap definitions, `trap_status()`, `integrity_score()` |
| `data/mod.rs` | 211 | Schema validation, gap detection, SHA256, `DatasetManifest` |

### Epistemic Path (~600 lines)
| Module | Lines | Purpose |
|--------|-------|---------|
| `epistemic.rs` | 536 | `EpistemicState::from_system()`, 8 epistemic levels, JSON export |
| `narrative_detector.rs` | 460 | 4 narrative regimes, reflexivity/bubble/panic detection |
| `regime.rs` | 328 | `classify_dataset()`, bridges detector with CsvRow data |
| `drift_tracker.rs` | 562 | 5 drift severities, position multiplier adjustment |

### Infrastructure (~3,000 lines)
| Module | Lines | Purpose |
|--------|-------|---------|
| `exchange/{binance,kraken}.rs` | 641 | REST API adapters |
| `feed/{binance_live,aux_data}.rs` | 775 | WebSocket streams, funding/borrow/liquidation |
| `reliability/{circuit,wal}.rs` | 726 | Circuit breaker, write-ahead log |
| `logging.rs` | 796 | 6 levels, 10 domains, structured JSONL |
| `live_ops.rs` | 302 | Fill reconciliation, circuit breaker integration |

### Dormant (~4,000 lines)
| Module | Status | Plan |
|--------|--------|------|
| `strategies.rs` (808 lines) | Dead code — 7 strategy types, none instantiated | Archive or benchmark in v2 |
| `signals.rs` (459 lines) | Dead code — consumed only by dead strategies.rs | Same |
| `filters.rs` (300 lines) | Dead code — not in backtest path | Same |

---

## 4. Binaries (21 executables)

### Production Pipeline
| Binary | Purpose | Output |
|--------|---------|--------|
| `backtest` | Run backtest on CSV data | stdout + JSON |
| `walk_forward` | Walk-forward validation with correction | `out/walk_forward/report.json` |
| `bench` | Profile timing and throughput | `out/bench/report.json` |
| `workbench` | Generate dashboard HTML | `docs/workbench.html` |
| `update_ledger` | Bayesian hypothesis updates | `out/ledger_history/updates.jsonl` |
| `coherence_check` | Data file validation | stdout |
| `dataset_manifest` | Dataset metadata | stdout |
| `epistemic_server` | HTTP API + dashboard server | `http://localhost:51723` |

### Diagnostic
| Binary | Purpose |
|--------|---------|
| `diagnose` | Strategy behavior deep-dive |
| `sweep` | Extended parameter sweep |
| `backtest_logged` | Backtest with structured logging |
| `event_backtest` | Event-driven strategy testing |
| `logparse` | Log analysis CLI |

### Stress & Verification
| Binary | Purpose |
|--------|---------|
| `stress` | Memory/latency stress test |
| `stress_parallel` | Concurrent safety test |
| `fuzz` | Random event stream chaos test |
| `replay` | Deterministic replay verification |
| `verify_coherence` | Multi-scale invariant checks |

### Infrastructure
| Binary | Purpose |
|--------|---------|
| `live_loop` | Async live trading loop |
| `skeleton_loop` | Minimal engine test loop |
| `permute` | Data permutation generator |

---

## 5. Guarantee Surface

### Guarded (9/18)
| # | Trap | Guard | Evidence |
|---|------|-------|----------|
| 1 | Survivorship bias | Guarded | All 12 strategies included in sweep |
| 2 | Look-ahead bias | Guarded | Sequential candle processing, no future data |
| 4 | Overfitting to noise | Guarded | Walk-forward validation with Bonferroni |
| 7 | Ignoring friction | Guarded | 4 exec modes with fee/slippage/latency |
| 8 | Unrealistic fills | Guarded | Fill probability model with adverse selection |
| 9 | Selection bias in reporting | Guarded | All strategies reported, not just winners |
| 10 | Data snooping | Guarded | Walk-forward train/test separation |
| 14 | Ignoring regime changes | Guarded | 4 regime datasets, regime classification |
| 15 | Cherry-picking timeframes | Guarded | 4 distinct market periods tested |

### Partial (4/18)
| # | Trap | Status | Gap |
|---|------|--------|-----|
| 3 | Look-ahead in features | Partial | Indicators are causal but funding data timing unclear |
| 11 | Parameter instability | Partial | 12 variants tested but no stability analysis |
| 16 | Ignoring market impact | Partial | Slippage model exists but uncalibrated |
| 17 | Unrealistic position sizes | Partial | Kelly sizing exists but not stress-tested |

### Unguarded (5/18)
| # | Trap | Risk |
|---|------|------|
| 5 | In-sample vs out-of-sample confusion | Walk-forward exists but not enforced in pipeline |
| 6 | Multiple comparisons without correction | Bonferroni applied but alpha threshold arbitrary |
| 12 | Backtest-to-live gap | No live trading data |
| 13 | Curve fitting | Parameter count vs data points not tracked |
| 18 | Ignoring tail risk | No extreme event simulation |

---

## 6. CI/CD Pipeline

### Workflows
| Workflow | Trigger | Jobs |
|----------|---------|------|
| `ci.yml` | Push to main, PR | Format → Clippy → Build → Tests (281) → Smoke (28) → Coherence |
| `pages.yml` | Push to main, dispatch | Copy docs → Deploy to GitHub Pages |

### Pipeline Script (`scripts/pipeline.sh`)
```
Step 1: Validate data (coherence_check)
Step 2: Backtest 4 regime datasets
Step 3: Walk-forward validation
Step 4: Update hypothesis ledger
Step 5: Bench profiling
Step 6: Generate workbench dashboard
Step 7: Summary report
```

### Live Deployments
| Surface | URL | Content |
|---------|-----|---------|
| GitHub Pages | https://uprootiny.github.io/arbitragefx/ | Static workbench dashboard |
| Epistemic Server | http://localhost:51723/ | Live dashboard + API |
| API Health | http://localhost:51723/api/health | System health JSON |
| API State | http://localhost:51723/api/state | Full epistemic state |

---

## 7. Subprojects and Roadmaps

### A. Backtest Realism (ROADMAP_BACKTEST.md)
**Status:** 30/52 tickets complete
**Focus:** Latency model, slippage calibration, partial fills, regime detection
**Next:** Calibrate slippage from real fill data (requires live trading)

### B. Hypothesis Research Cycle (ROADMAP_NEXT_RELEASE.md)
**Status:** v0.2.0 complete, v0.3.0 in progress
**Focus:** Automated Bayesian updates, walk-forward pipeline, session templates
**Next:** Session templates for structured experiments

### C. Observation Surface (this session)
**Status:** Phase 1 complete (6/6 items)
**Phase 1 delivered:** Evidence timeline, trap checklist, uncertainty kanban, regime leaderboard, resource trends, enriched health API
**Phase 2 planned:** Ontology graph, session templates, reconciliation workflow, activity feed, API enrichment
**Phase 3 planned:** Evidence provenance, reproduce binary

### D. Live Readiness (live_readiness.md)
**Status:** Layer 1-2 partial, Layers 3-8 not started
**Dependency stack:** Data integrity → Order lifecycle → Execution → Risk → Persistence → Gatekeeping → Observability → Deployment

---

## 8. Data Schema

### Input (CSV, 11 columns)
```
timestamp,open,high,low,close,volume,funding_rate,borrow_rate,liquidation_score,stable_depeg,open_interest
```

### Output (BacktestResult JSON)
```json
{
  "total_pnl": -1.23,
  "max_drawdown": 0.0082,
  "buy_hold_pnl": -270.45,
  "strategies": [
    {"id": "churn-0", "pnl": 2.34, "equity_pnl": -0.56, "equity": 99.44,
     "friction": 2.90, "max_drawdown": 0.0041, "trades": 12, "wins": 7, "losses": 5}
  ],
  "config_hash": "a1b2c3d4...",
  "candle_count": 1000
}
```

### Evidence (JSONL, append-only)
```json
{"ts":"...","git_sha":"c5f9512","hypothesis_id":"H001",
 "old_stv":[0.42,0.72],"new_stv":[0.46,0.78],
 "dataset":"auto","observation":"7/12 positive in test","supports":"false"}
```

---

## 9. Configuration

55 parameters via environment variables. No config file. No CLI args.

### Critical Parameters
| Parameter | Default | Effect |
|-----------|---------|--------|
| `ENTRY_THRESHOLD` | 1.0 | Score threshold for trade entry |
| `EDGE_SCALE` | 0.002 | Expected edge per trade |
| `TAKE_PROFIT` | 0.01 | Target profit per trade |
| `STOP_LOSS` | 0.006 | Maximum loss per trade |
| `MAX_POSITION_PCT` | 0.10 | Max portfolio fraction per position |
| `MIN_HOLD_CANDLES` | 3 | Minimum hold period (anti-overtrading) |
| `FEE_RATE` | 0.001 | Per-notional fee |
| `SLIPPAGE_K` | 0.0005 | Slippage coefficient |

### Config Hash
SHA256 of sorted key=value pairs. Same hash = same configuration = reproducible result (given deterministic execution).

---

## 10. Ecosystem Position

ArbitrageFX operates within the Shevat ecosystem alongside:
- **honeycomb** — observation surface (port 7777), project constellation grid
- **atlas/dance** — dashboard and project navigation
- **fancy** — liveness registry with event sourcing
- **infra** — deployment manifests and server configuration
- **canon** — foundation documents, philosophy, guidelines
- **journal** — development chronicle (daily/weekly/monthly)
- **theirtents** — ClojureScript UI layer with zone-based architecture

### Interoperability Points
- ArbitrageFX `/api/health` can feed into **fancy** liveness registry
- Hypothesis ledger can be visualized in **honeycomb** constellation
- Backtest results can be chronicled in **journal**
- Infrastructure deployment via **infra** rsync manifests
- UI layer could use **theirtents** zone architecture for ClojureScript frontend

---

*Generated from codebase analysis at git SHA a83abaf. 15,700 lines of library code, 4,000 lines of binaries, 281 tests passing.*
