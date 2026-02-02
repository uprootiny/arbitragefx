# ArbitrageFX Blueprint
## A Survey of Where We Are, Where We're Going, and Why

*"Before acting, one should know: What is the purpose? What are the means? What are the consequences?"*

---

## I. Current State Survey

### Core System (Rust)
```
src/
├── hypothesis.rs    ✓ Hypothesis ledger with refinement tracking
├── signals.rs       ✓ Composable signal generators (8 types)
├── indicators.rs    ✓ Technical indicators (EMA, RSI, MACD, BB, ATR, Stochastic)
├── filters.rs       ✓ Trade filters (volatility, trend, position limits)
├── sizing.rs        ✓ Position sizing (fixed, Kelly, volatility-adjusted)
├── strategies.rs    ✓ Strategy implementations (7 types)
├── backtest.rs      ✓ Backtesting engine
├── logging.rs       ✓ Structured JSONL logging
├── risk.rs          ✓ Risk management
├── engine/          ✓ Live trading engine
└── exchange/        ✓ Exchange adapters (Binance, Kraken)
```

### Research Tools
```
research_lab        ✓ Hypothesis testing CLI
strategy_lab        ✓ Multi-strategy comparison
logparse            ✓ Log analysis and replay
verify_coherence    ✓ Multi-scale verification
```

### Hypothesis Status (StrongBear -28.4%)
| ID | Hypothesis | Sharpe | Status |
|----|------------|--------|--------|
| H006 | Short-selling in bear markets | 1.96 | SUPPORTED |
| H004 | Trend filtering improves quality | 0.62 | SUPPORTED |
| H008 | Volatility breakouts tradeable | 0.45 | SUPPORTED |
| H001-H003,H005,H007 | Various | <0.4 | Testing |

### Gaps Identified
- [ ] Bull market data needed for regime coverage
- [ ] Ranging market data for mean reversion tests
- [ ] Composite strategy combining supported hypotheses
- [ ] Paper trading validation before live

---

## II. Ethical Foundation

### The Five Precepts Applied to Trading

1. **Non-harm (ahiṃsā)**
   - No market manipulation
   - No exploitation of information asymmetry beyond fair service
   - Position limits to prevent market impact

2. **Non-stealing (asteya)**
   - Only profit from genuine market inefficiencies
   - No front-running or information theft
   - Transparent fee structures

3. **Truthfulness (satya)**
   - Accurate logging of all decisions
   - Honest performance reporting
   - No self-deception in strategy evaluation

4. **Non-greed (aparigraha)**
   - Profit caps and drawdown limits
   - Kelly fraction ≤ 0.25 (conservative sizing)
   - Contentment with reasonable returns

5. **Right Livelihood (sammā-ājīva)**
   - The system serves price discovery
   - Provides liquidity where needed
   - Operates within legal/regulatory bounds

### Calibration as Practice

The calibration game is not just skill training—it's a meditation on:
- **Impermanence**: Markets change; certainty is illusion
- **Dependent origination**: Prices arise from conditions
- **Non-self**: "My prediction" is just pattern recognition, not ego

When you say "70% confident" and track whether you're right 70% of the time,
you practice *seeing things as they are* (yathā-bhūta-ñāṇa-dassana).

---

## III. Project Hygiene

### Active Projects
| Project | Status | Last Commit | Health |
|---------|--------|-------------|--------|
| arbitragefx | Active | Today | ✓ 158 tests pass |
| calibration-game | Scaffolded | Today | Needs ClojureScript build |
| trajectory-explorer | Scaffolded | Today | Needs ClojureScript build |

### Archived Projects (Rescued)
| Project | Value | Action |
|---------|-------|--------|
| tinymon | Monitoring patterns | Extract learnings ✓ |
| calibration_game | Forecasting game | Modernized → calibration-game |
| financial_timeline_elixir | Actor architecture | Study for future scaling |

### Disk Usage
- Before cleanup: 6.4GB free (98% full)
- After cleanup: 36GB free (88% full)
- Rust targets cleaned: ~22GB
- Node modules cleaned: ~2.5GB
- Python venvs cleaned: ~1.5GB

---

## IV. Development Trajectory

### Phase 1: Foundation ✓
- [x] Core backtesting engine
- [x] Signal/filter/sizing framework
- [x] Hypothesis ledger
- [x] Logging infrastructure

### Phase 2: Validation (Current)
- [x] Test in StrongBear regime
- [ ] Test in StrongBull regime
- [ ] Test in Ranging regime
- [ ] Paper trading validation

### Phase 3: Integration
- [ ] Composite adaptive strategy
- [ ] Regime detection module
- [ ] Live trading with circuit breakers
- [ ] Monitoring dashboard

### Phase 4: Operation
- [ ] Gradual capital deployment
- [ ] Continuous hypothesis refinement
- [ ] Performance review cycles
- [ ] Ethical audit quarterly

---

## V. Daily Practice

### Morning Review
```bash
# Check hypothesis status
./target/release/research_lab hypotheses

# See recommended actions
./target/release/research_lab actions

# Run calibration practice (5 predictions)
# (when calibration-game is built)
```

### Before Any Trade
Ask:
1. What hypothesis supports this?
2. What is my confidence level?
3. What would refute my belief?
4. Is this right livelihood?

### Evening Reflection
- Review day's decisions in logs
- Update hypothesis ledger with evidence
- Note any calibration errors
- Practice gratitude for what worked

---

## VI. The Middle Way

Neither:
- Reckless speculation (greed, FOMO)
- Fearful inaction (aversion, analysis paralysis)

But:
- **Calibrated confidence**: Know what you know
- **Tested belief**: Hypothesize, test, refine
- **Right-sized risk**: Kelly fraction, position limits
- **Continuous learning**: Every trade is data

The system is not about maximizing profit.
It's about *right relationship* with uncertainty.

---

## VII. Next Actions

Immediate:
1. Acquire bull market data (2020-2021 BTC)
2. Test supported hypotheses in new regime
3. Build calibration-game ClojureScript frontend

This Week:
4. Design composite strategy from H004+H006+H008
5. Implement regime detection
6. Paper trading setup

This Month:
7. 30 days paper trading validation
8. Performance review
9. Decision: proceed to live or refine further

---

*"The wise one, surveying the world with wisdom's eye,
sees what is beneficial and what is harmful,
and chooses the path that leads to lasting welfare."*

— Adapted from the Dhammapada
