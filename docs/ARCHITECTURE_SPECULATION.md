# Architectural Speculation

**Date**: 2026-02-01
**Status**: Exploratory

---

## 1. The Friction Problem: A Structural Analysis

### Current State

The system exhibits a consistent pattern: **strategies are profitable before friction, unprofitable after**. This isn't a bug—it's a fundamental constraint of market microstructure.

```
Friction = Fees + Slippage + Spread

For a trade to be profitable:
Expected_Edge > Friction × 2  (entry + exit)
```

### Structural Solution: Maker-Only Execution

Rather than treating friction as a parameter to optimize, we should **change the execution model**:

```
Current:  Signal → Market Order → Immediate Fill → Pay Taker Fee
Proposed: Signal → Limit Order → Queue for Fill → Earn Maker Rebate
```

**Trade-off**: Latency for cost reduction. We sacrifice fill certainty for edge preservation.

### Implementation Sketch

```rust
enum ExecutionMode {
    Aggressive,  // Market orders, guaranteed fill
    Passive,     // Limit at mid, may not fill
    Adaptive,    // Switch based on urgency score
}

struct PassiveOrder {
    limit_price: f64,      // Mid price at signal time
    time_in_force: u64,    // Cancel after N seconds
    chase_threshold: f64,  // Chase if price moves X%
}
```

---

## 2. The Timeframe Paradox

### Observation

Short timeframes (5m): More signals, more noise, friction dominates
Long timeframes (1h): Fewer signals, cleaner trends, but stale entries

### Hypothesis: Multi-Resolution Architecture

What if the system ran multiple timeframe instances in parallel?

```
┌─────────────────────────────────────────────┐
│              Ensemble Controller            │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐     │
│  │ 5m Inst │  │ 15m Inst│  │ 1h Inst │     │
│  │ Scalp   │  │ Trend   │  │ Position│     │
│  └────┬────┘  └────┬────┘  └────┬────┘     │
│       │            │            │           │
│       v            v            v           │
│  ┌─────────────────────────────────────┐   │
│  │         Signal Aggregator           │   │
│  │  vote = sum(weight × confidence)    │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

### Benefits
- Short TF catches early entries
- Long TF provides trend confirmation
- Disagreement = stay flat

### Risks
- Complexity explosion
- Conflicting signals
- 3× data feed requirements

---

## 3. The Edge Hurdle Discovery

### Empirical Finding

```
edge_hurdle=0.001 → 750 trades, -$52.51
edge_hurdle=0.008 → 84 trades, +$13.00
edge_hurdle=0.012 → 3 trades, +$20.16
```

### Theoretical Implication

This suggests the signal distribution is heavy-tailed:
- Many weak signals (noise)
- Few strong signals (alpha)

A rational edge hurdle should be set where:
```
Expected[PnL | signal > hurdle] > Expected[Friction]
```

### Speculation: Dynamic Hurdle

What if edge_hurdle adapted to recent signal quality?

```rust
struct AdaptiveHurdle {
    base: f64,
    recent_accuracy: RingBuffer<bool>,  // Win/loss history

    fn current(&self) -> f64 {
        let accuracy = self.recent_accuracy.mean();
        if accuracy > 0.6 {
            self.base * 0.8  // Lower hurdle, signals are good
        } else if accuracy < 0.4 {
            self.base * 1.5  // Raise hurdle, signals are bad
        } else {
            self.base
        }
    }
}
```

---

## 4. Position Sizing Paradox Resolution

### The Mystery

Small positions perform worse than large on 1-hour data. This violates Kelly criterion intuition.

### Hypothesis: Fixed Friction per Trade

If friction is dominated by **per-trade costs** (API calls, spread) rather than **proportional costs** (% fees):

```
Trade 0.001 BTC: friction = $0.10 base + 0.1% × $90 = $0.19
Trade 0.01 BTC:  friction = $0.10 base + 0.1% × $900 = $1.00

Friction as % of notional:
  Small: $0.19 / $90 = 0.21%
  Large: $1.00 / $900 = 0.11%
```

Large positions have **lower friction percentage**.

### Architectural Implication

Position size should scale with expected edge magnitude, not be fixed:

```rust
fn optimal_position_size(expected_edge: f64, friction_per_trade: f64) -> f64 {
    // Only trade if edge exceeds friction threshold
    if expected_edge < friction_per_trade * 2.5 {
        return 0.0;
    }

    // Kelly fraction scaled by confidence
    let kelly = (expected_edge - friction_per_trade) / variance;
    kelly.min(max_position_pct)
}
```

---

## 5. The Determinism-Latency Trade-off

### Current Design

The system is fully deterministic: same inputs → same outputs. This enables:
- Replay debugging
- Audit trails
- Backtesting fidelity

### But Live Execution is Non-Deterministic

Real markets have:
- Random fill times
- Price slippage
- Network jitter
- Exchange matching engine behavior

### Speculation: Probabilistic Execution Model

What if we embraced non-determinism at the execution layer while maintaining determinism at the signal layer?

```
┌────────────────────────────────────────────┐
│  Deterministic Core (replay-safe)          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │  Feed    │→│  Strategy │→│   Risk   │  │
│  │  Replay  │  │  Engine   │  │  Engine  │  │
│  └──────────┘  └──────────┘  └──────────┘ │
└────────────────────┬───────────────────────┘
                     │ Action
                     v
┌────────────────────────────────────────────┐
│  Probabilistic Execution (live-only)       │
│  - Randomized order timing                 │
│  - Adaptive limit price adjustment         │
│  - Partial fill handling                   │
└────────────────────────────────────────────┘
```

---

## 6. Alternative Architectures

### 6.1 Event Sourcing + CQRS

Separate read (market data) and write (order execution) paths completely:

```
Candles → Event Store → Projections → Query Model
                    ↓
              Command Handler → Order Commands → Exchange
```

**Benefit**: Clean separation, easy replay
**Cost**: Complexity, eventual consistency

### 6.2 Actor Model

Each strategy as an independent actor:

```rust
enum StrategyMsg {
    Candle(Candle),
    Fill(Fill),
    RiskAlert(Alert),
    Shutdown,
}

impl Actor for StrategyActor {
    fn receive(&mut self, msg: StrategyMsg) -> Option<Action> {
        // ...
    }
}
```

**Benefit**: Isolation, parallelism
**Cost**: Message passing overhead, debugging difficulty

### 6.3 Functional Core, Imperative Shell

Pure functions for all business logic, IO at edges:

```rust
// Pure
fn compute_action(state: &State, candle: &Candle) -> (Action, State) {
    // No side effects
}

// Imperative shell
async fn run_loop(adapter: &mut Adapter) {
    loop {
        let candle = adapter.recv_candle().await;
        let (action, new_state) = compute_action(&state, &candle);
        adapter.submit_order(action).await;
        state = new_state;
    }
}
```

**Benefit**: Testability, reasoning
**Cost**: Awkward async integration

---

## 7. Unexplored Questions

1. **Cross-venue arbitrage**: Can we profit from price differences between Binance spot and futures?

2. **Funding rate capture**: Is pure funding arbitrage (delta-hedged) more profitable than directional trading?

3. **Regime classification**: Can we detect regime changes faster than the EMA crossover method?

4. **Ensemble methods**: Would multiple weak strategies combined outperform one strong strategy?

5. **Adversarial robustness**: How does the strategy perform if someone is actively trading against it?

---

## 8. Next Steps

1. **Prototype maker-only execution** - Test limit order fill rates
2. **Backtest multi-timeframe ensemble** - Combine 5m + 1h signals
3. **Implement dynamic edge hurdle** - Adapt based on recent accuracy
4. **Calibrate slippage model** - Use live fill data

---

## Appendix: Back-of-Envelope Calculations

### Break-Even Trade Frequency

```
Given:
  - Maker fee: 0.02%
  - Average edge: 0.1% per trade
  - Win rate: 55%

Expected PnL per trade:
  = 0.55 × 0.1% - 0.45 × 0.1% - 2 × 0.02%
  = 0.055% - 0.045% - 0.04%
  = -0.03%  # Still negative!

Need either:
  - Higher win rate (>70%), or
  - Larger average edge (>0.15%), or
  - Lower fees (maker rebate)
```

### Optimal Trade Frequency

```
If edge decays with frequency:
  edge(n) = base_edge / sqrt(n)

And friction is constant per trade:
  friction = 0.02%

Optimal n where marginal edge = friction:
  base_edge / sqrt(n) = friction
  n = (base_edge / friction)^2

With base_edge=0.5% and friction=0.02%:
  n = (0.5/0.02)^2 = 625 trades per period

On 1000 bars, this suggests ~0.6 trades per bar optimal.
```
