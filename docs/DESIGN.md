# System Design Document

**Project**: arbitragefx
**Version**: 0.1.0
**Status**: Development

---

## 1. Overview

### 1.1 Purpose

arbitragefx is an ethically-aligned cryptocurrency trading system that:
- Executes momentum and mean-reversion strategies on crypto perpetual markets
- Integrates Buddhist ethics framework to prevent harmful trading patterns
- Maintains deterministic replay capability via WAL-based state management
- Supports both backtesting and live execution modes

### 1.2 Design Principles

1. **Determinism**: Same inputs → same outputs (for replay/audit)
2. **Simplicity**: Unix philosophy - do one thing well
3. **Safety**: Fail-safe defaults, position limits, circuit breakers
4. **Observability**: Structured logging, metrics, clear state transitions

---

## 2. Architecture

### 2.1 Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Main Loop                                │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │  Feed   │→│  State  │→│Strategy │→│  Risk   │→│ Adapter │  │
│  │ Manager │  │ Manager │  │ Engine  │  │ Engine  │  │ (Exec)  │  │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │
│       ↑                                                    ↓      │
│  ┌─────────┐                                         ┌─────────┐  │
│  │   WAL   │←────────────────────────────────────────│  Fills  │  │
│  └─────────┘                                         └─────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

```
Candle → Indicators → MarketView → Strategy.update() → Action
                                                         ↓
                                   RiskEngine.apply() ← Action
                                         ↓
                                   GuardedAction → Adapter.submit()
                                                         ↓
                                   Fill → Portfolio.apply_fill()
                                         ↓
                                   Metrics.update()
```

### 2.3 Module Responsibilities

| Module | Responsibility | Key Types |
|--------|----------------|-----------|
| `state.rs` | Configuration, market state, strategy instances | `Config`, `MarketState`, `StrategyInstance` |
| `strategy.rs` | Trading logic, signal generation | `Action`, `MarketView`, `PortfolioState` |
| `risk.rs` | Position limits, guards, circuit breakers | `RiskEngine`, `RiskVerdict` |
| `adapter/` | Exchange abstraction | `UnifiedAdapter`, `OrderRequest` |
| `feed/` | Market data ingestion | `Candle`, `MarketAux` |
| `engine/` | Core loop, backtest, logging | `engine_loop`, `Ledger`, `Journal` |

---

## 3. Strategy Design

### 3.1 Signal Generation

The SimpleMomentum strategy generates signals based on:

```
score = 1.0 × z_momentum
      + 0.3 × z_vol
      + 0.5 × z_volume_spike
      + stretch_contrib (conditional on trend)

if score > entry_threshold AND trend_confirms:
    → Buy
if score < -entry_threshold AND trend_confirms:
    → Sell
```

### 3.2 Trend Filter

```rust
in_uptrend = ema_fast > ema_slow
in_downtrend = ema_fast < ema_slow
strong_trend = |ema_fast - ema_slow| / ema_slow > 0.01

// Mean reversion only allowed when:
// - Aligned with trend, OR
// - Trend is weak
```

### 3.3 Exit Conditions

| Condition | Action |
|-----------|--------|
| move_pct >= take_profit | Close |
| move_pct <= -stop_loss | Close |
| elapsed >= time_stop | Close |
| score < exit_threshold | Close |

---

## 4. Risk Management

### 4.1 Position Limits

```rust
max_position = equity × max_position_pct / price
// Default: 5% of equity
```

### 4.2 Circuit Breakers

| Trigger | Response |
|---------|----------|
| 3 consecutive losses | Pause 1 hour |
| 5 losses in rolling window | Halt trading |
| Fill slippage > 2% | Halt trading |
| Reconciliation drift > 2% | Halt trading |

### 4.3 Ethical Guards (Three Poisons)

| Poison | Guard | Threshold |
|--------|-------|-----------|
| Greed | Overtrading | > 10 trades/day |
| Aversion | Revenge trading | < 5 min since loss |
| Delusion | Noise trading | score < edge_hurdle |

---

## 5. State Management

### 5.1 WAL Structure

```rust
enum WalEntry {
    Intent { ts, strategy_id, action, params_hash },
    Submit { ts, intent_id, order_id, params_hash },
    Fill { ts, intent_id, params_hash, price, qty, fee },
    Cancel { ts, intent_id, params_hash },
    Snapshot { ts, strategy_id, state_json },
}
```

### 5.2 Recovery Protocol

1. Read WAL from last valid entry
2. Replay events to rebuild state
3. Reconcile with exchange via API
4. Resume from consistent state

---

## 6. Configuration

### 6.1 Critical Parameters

| Parameter | Default | Range | Impact |
|-----------|---------|-------|--------|
| entry_threshold | 1.2 | 0.8-2.5 | Trade frequency |
| edge_hurdle | 0.003 | 0.001-0.012 | Signal quality |
| stop_loss | 0.004 | 0.002-0.008 | Risk per trade |
| take_profit | 0.006 | 0.003-0.015 | Reward target |
| position_size | 0.001 | 0.0005-0.01 | Capital at risk |

### 6.2 Timeframe Recommendations

| Timeframe | entry_threshold | edge_hurdle | stop_loss |
|-----------|-----------------|-------------|-----------|
| 1-5 min | 1.0-1.5 | 0.002-0.004 | 0.003 |
| 15-30 min | 1.5-2.0 | 0.004-0.006 | 0.005 |
| 1-4 hour | 2.0-2.5 | 0.008-0.012 | 0.008 |

---

## 7. Testing Strategy

### 7.1 Test Levels

| Level | Scope | Coverage |
|-------|-------|----------|
| Unit | Individual functions | 80%+ |
| Integration | Module interactions | 60%+ |
| Backtest | Full system on historical | 90%+ |
| Paper | Full system live (no real money) | 100% |

### 7.2 Key Invariants

```rust
// Portfolio
assert!(cash + position × price ≈ equity)
assert!(position.abs() <= max_position)

// Strategy
assert!(trades_today <= daily_limit)
assert!(last_trade_ts >= last_loss_ts || cooldown_elapsed)

// Risk
assert!(drawdown <= max_drawdown_limit)
```

---

## 8. Deployment

### 8.1 Environment Variables

```bash
# Required
BINANCE_API_KEY=xxx
BINANCE_API_SECRET=xxx
SYMBOL=BTCUSDT

# Recommended
ENTRY_TH=1.5
EDGE_HURDLE=0.006
POSITION_SIZE=0.001
```

### 8.2 Startup Checklist

- [ ] API keys configured
- [ ] WAL path writable
- [ ] Kill file path accessible
- [ ] Network connectivity to exchange
- [ ] Sufficient balance for initial position

---

## 9. Future Work

1. **Multi-exchange**: Support Kraken, OKX alongside Binance
2. **Multi-asset**: Portfolio optimization across BTC, ETH, SOL
3. **ML Integration**: Learned entry/exit thresholds
4. **Real-time Dashboard**: Grafana/Prometheus metrics
5. **Automated Rebalancing**: Cross-venue arbitrage

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| Edge Hurdle | Minimum expected profit to take a trade |
| Z-score | Standard deviations from rolling mean |
| Regime | Market condition (trending vs ranging) |
| WAL | Write-Ahead Log for crash recovery |
| Three Poisons | Buddhist concept: greed, aversion, delusion |
