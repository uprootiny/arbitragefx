# arbitragefx Architecture

**Version**: 0.1.0
**Date**: 2026-02-01
**LOC**: ~12,500

---

## Design Philosophy

This system follows three core principles:

1. **Deterministic Replay**: Every state mutation flows through typed events, enabling crash recovery and debugging via replay.
2. **Ethical Trading Constraints**: Buddhist-inspired guards (Three Poisons) prevent greed, fear, and ignorance from corrupting trading decisions.
3. **Unix Philosophy**: Small, composable modules with clear contracts. Text-based interfaces where possible.

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            LIVE LOOP (main.rs)                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
│  │  Exchange   │───▶│  EventBus   │───▶│   Reducer   │                 │
│  │  Adapters   │    │             │    │             │                 │
│  └─────────────┘    └─────────────┘    └──────┬──────┘                 │
│        │                                      │                         │
│        │                               ┌──────▼──────┐                 │
│        │                               │EngineState  │                 │
│        │                               │  + hash()   │                 │
│        │                               └──────┬──────┘                 │
│        │                                      │                         │
│        │                               ┌──────▼──────┐                 │
│        │                               │   Policy    │                 │
│        │                               │  (Intent)   │                 │
│        │                               └──────┬──────┘                 │
│        │                                      │                         │
│        │    ┌─────────────┐            ┌──────▼──────┐                 │
│        │◀───│    WAL      │◀───────────│  RiskEngine │                 │
│        │    │  Recovery   │            │  (Guards)   │                 │
│        │    └─────────────┘            └──────┬──────┘                 │
│        │                                      │                         │
│        ▼                               ┌──────▼──────┐                 │
│  ┌─────────────┐                       │   Command   │                 │
│  │   Fills     │◀──────────────────────│ (PlaceOrder)│                 │
│  │  Channel    │                       └─────────────┘                 │
│  └─────────────┘                                                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Core Modules

### 1. Event System (`src/engine/`)

**Components**:
- `bus.rs` - Typed event queue with priority ordering
- `events.rs` - Event enum (Market, Fill, Risk, Sys)
- `reducer.rs` - Pure function: (State, Event) → (State, Commands)

**Data Flow**:
```
Event → Reducer → State' + Commands → Executor → Exchange
                    ↓
              state.hash() → WAL
```

**Event Types**:
```rust
enum Event {
    Market(MarketEvent),   // Candles, aux data, prices
    Fill(FillEvent),       // Order fills from exchange
    Risk(RiskEvent),       // Circuit breaker, halt signals
    Sys(SysEvent),         // Timers, lifecycle events
}
```

### 2. State Management (`src/state.rs`, `src/engine/state.rs`)

**MarketState**: Holds per-symbol candle buffers and indicators
- `RingBuffer<Candle>` - Rolling window of OHLCV data
- `IndicatorState` - Welford online variance for z-scores
- `MarketAux` - External data (funding, liquidations, depeg)

**StrategyState**: Per-strategy instance state
- `PortfolioState` - Cash, position, entry price, equity
- `MetricsState` - Wins, losses, PnL, drawdown
- Trading metadata (last trade, cooldown, daily limits)

**EngineState**: Aggregated reducer state
- Current regime (narrative detector)
- Drift tracker statistics
- Position exposure map

### 3. Strategy Layer (`src/strategy.rs`, `src/state.rs`)

**Strategy Trait**:
```rust
pub trait Strategy {
    fn id(&self) -> &'static str;
    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action;
    fn aux_requirements(&self) -> AuxRequirements;
}
```

**Concrete Strategies**:
- `SimpleMomentum` - Z-score based with vol regime switching
- `CarryOpportunistic` - Funding carry + event bursts

**Action Enum**:
```rust
enum Action {
    Hold,
    Buy { qty: f64 },
    Sell { qty: f64 },
    Close,
}
```

### 4. Policy Layer (`src/engine/policy.rs`)

**Intent Abstraction**: Strategies express *what* they want, not *how*
```rust
pub enum Intent {
    TargetExposure { symbol: String, target_delta: f64 },
    Flat { symbol: String },
    ReduceRisk { symbol: String, reduction_factor: f64 },
    Hold,
    RequestHalt { reason: String },
}
```

**Policy Trait**:
```rust
pub trait Policy: Send + Sync {
    fn decide(&self, input: &AgentInput) -> AgentOutput;
    fn name(&self) -> &str;
}
```

### 5. Risk Engine (`src/risk.rs`)

**Guards**:
1. **Position Limit** - Max % of equity per position
2. **Daily Loss Limit** - Max % drawdown per day
3. **Trade Frequency** - Max trades per day
4. **Cooldown** - Minimum time between trades
5. **Volatility Pause** - Halt on vol spikes
6. **Halt Flag** - Emergency stop

**Ethics Integration**:
- `apply_with_price()` includes MTM unrealized PnL
- Always allows `Action::Close` through guards (don't trap losses)

### 6. Ethics Framework (`src/engine/ethics.rs`, `src/engine/eightfold_path.rs`)

**Three Poisons → Concrete Guards**:
| Poison | Symptom | Guard |
|--------|---------|-------|
| Greed | Over-leveraging | Position limits |
| Aversion | Panic selling | Cooldown timers |
| Delusion | Stale data | Freshness checks |

**Eightfold Path Checklist**:
- Right Action: Size within limits
- Right Livelihood: No market manipulation
- Right Effort: Respect rate limits
- Right Mindfulness: Data freshness checks
- Right Concentration: Single clear intent per decision

### 7. Reliability (`src/reliability/`)

**WAL (Write-Ahead Log)**:
- `wal.rs` - Append-only journal with snapshots
- Per-strategy snapshot recovery
- Fill replay for crash recovery

**Recovery Flow**:
```
1. Load WAL entries
2. Find last snapshot per strategy
3. Replay fills since snapshot
4. Verify state hash matches
5. Resume from recovered state
```

### 8. Drift Detection (`src/engine/drift_tracker.rs`)

**Purpose**: Detect when live market deviates from backtest assumptions

**Tracked Metrics**:
- Volatility ratio
- Return distribution
- Spread widening
- Funding extremes
- Z-score drift

**Severity Levels**:
```rust
enum DriftSeverity {
    None,
    Low,
    Moderate,
    Severe,
    Critical,
}
```

**Response**: Position multiplier scales down as drift increases

### 9. Exchange Adapters (`src/exchange/`, `src/adapter/`)

**Binance**:
- `binance.rs` - REST API (candles, orders, funding)
- `binance_live.rs` - WebSocket for live fills

**Kraken**:
- `kraken.rs` - REST API adapter

**Common Interface**:
```rust
pub trait Exchange {
    fn candles(&self, ...) -> Result<Vec<Candle>>;
    fn place_order(&self, ...) -> Result<OrderResponse>;
    fn open_orders(&self) -> Result<Vec<Order>>;
    fn account_balance(&self) -> Result<f64>;
}
```

### 10. Backtesting (`src/backtest.rs`)

**Execution Model**:
- Slippage: `k * (qty/liquidity) * (1 + vol*2)`
- Latency: Deterministic delay based on timestamp
- Partial fills: `max_fill_ratio` per bar

**Output**:
- Per-strategy PnL, equity curve
- Friction costs (fees + slippage)
- Trade counts, win/loss ratio
- Forced close count

---

## Data Flow Diagrams

### Candle Processing
```
Exchange.candles() → MarketState.on_candle() → IndicatorState.update()
                                                       ↓
                                              IndicatorSnapshot
                                                       ↓
                                              Strategy.update()
                                                       ↓
                                                    Action
```

### Order Lifecycle
```
Strategy.Action → RiskEngine.apply() → Command.PlaceOrder
                                              ↓
                                         WAL.append()
                                              ↓
                                      Exchange.place_order()
                                              ↓
                                         FillChannel
                                              ↓
                                    PortfolioState.apply_fill()
```

### Recovery Flow
```
WAL.read() → RecoveryState
                  ↓
           snapshots_by_strategy (HashMap)
                  ↓
           fills_since_snapshot (Vec)
                  ↓
           StrategyInstance.restore() + apply_fills()
                  ↓
           verify_hash() → Resume
```

---

## Module Dependencies

```
main.rs
├── state.rs (Config, MarketState, StrategyInstance)
├── strategy.rs (Strategy trait, MarketView, StrategyState)
├── risk.rs (RiskEngine)
├── backtest.rs (run_backtest)
├── logging.rs (json_log)
├── metrics.rs (MetricsEngine)
├── features.rs (FeaturePipeline)
├── events.rs (detect_phase1)
├── engine/
│   ├── bus.rs (EventBus)
│   ├── events.rs (Event, Command)
│   ├── reducer.rs (reduce)
│   ├── state.rs (EngineState)
│   ├── policy.rs (Intent, Policy)
│   ├── ethics.rs (EthicsCheckpoint)
│   ├── eightfold_path.rs (PathValidator)
│   ├── drift_tracker.rs (DriftTracker)
│   └── narrative_detector.rs (NarrativeRegime)
├── reliability/
│   └── wal.rs (Wal, WalEntry, RecoveryState)
├── exchange/
│   ├── mod.rs (Exchange trait, Candle)
│   ├── binance.rs (BinanceExchange)
│   ├── kraken.rs (KrakenExchange)
│   ├── signing.rs (HMAC-SHA256)
│   └── retry.rs (exponential backoff)
├── feed/
│   ├── aux_data.rs (AuxDataFetcher, caching)
│   └── binance_live.rs (WebSocket listener)
└── adapter/
    └── binance.rs (adapter layer)
```

---

## Invariants & Contracts

### State Invariants
1. `position == 0` implies `entry_price == 0`
2. `equity == cash + (position * price)`
3. `max_drawdown <= 0` (always negative or zero)
4. `wins + losses == total_trades`

### WAL Invariants
1. Snapshot hashes are deterministic given same state
2. Fills can be replayed to reach same state hash
3. Per-strategy snapshots are independent

### Risk Invariants
1. `Action::Close` always passes through guards
2. MTM PnL includes unrealized losses
3. Position limits respect current equity (not initial)

### Strategy Invariants
1. `aux_requirements()` returns consistent values
2. Missing aux data → `Action::Hold`
3. Stale aux data → `Action::Hold`

---

## Configuration

Environment variables (see `Config::from_env()`):

| Variable | Default | Description |
|----------|---------|-------------|
| SYMBOL | BTCUSDT | Trading pair |
| CANDLE_SECS | 300 | Candle granularity |
| MAX_POS_PCT | 0.05 | Max position as % of equity |
| MAX_DAILY_LOSS_PCT | 0.02 | Daily loss limit |
| COOLDOWN_SECS | 600 | Post-trade cooldown |
| VOL_PAUSE_MULT | 2.5 | Vol spike threshold |
| TAKE_PROFIT | 0.006 | Take profit % |
| STOP_LOSS | 0.004 | Stop loss % |
| WAL_PATH | ./bot.wal | WAL file location |
| RECONCILE_SECS | 60 | Fill reconciliation interval |

---

## Testing Strategy

### Well-Tested (see ENGINEERING_REVIEW.md)
- `engine/reducer` - Pure function, deterministic
- `strategy` - MarketAux validation
- `risk` - Guard logic
- `engine/policy` - Intent resolution
- `feed/aux_data` - Caching, staleness

### Needs Tests (P0/P1)
- `state.rs` - Strategy signal logic (now 23 tests added)
- `backtest.rs` - Execution model validation
- WAL crash → replay → hash match integration
- Circuit breaker behavior

---

## Security Considerations

1. **API Keys**: Environment variables only, never logged
2. **Signing**: HMAC-SHA256 for authenticated requests
3. **Rate Limiting**: Exponential backoff on failures
4. **Input Validation**: CSV range checks, division guards

---

## Future Work

1. **Multi-exchange**: Unified order routing
2. **Portfolio optimization**: Kelly criterion sizing
3. **Real-time dashboard**: Position monitoring
4. **Paper trading mode**: Full simulation without funds at risk
