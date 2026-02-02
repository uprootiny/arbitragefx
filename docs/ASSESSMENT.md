# System Assessment

**Date**: 2026-02-01
**Assessor**: Automated Sweep Analysis
**Version**: v0.1.0

---

## Executive Summary

The arbitragefx system shows **conditional profitability** depending on:
1. Fee structure (maker vs taker)
2. Timeframe selection (5m vs 1h)
3. Entry selectivity (edge hurdle)
4. Position sizing

**Key Finding**: The system can be profitable but requires careful parameter tuning.

---

## Quantitative Assessment

### Backtest Results Summary

| Timeframe | Best Config | Equity Δ | Win Rate | Trades | Buy-Hold |
|-----------|-------------|----------|----------|--------|----------|
| 5-min | opt_swing | +$11.81 | 66.7% | 27 | -$11,357 |
| 15-min | fee_zero | +$0.66 | - | - | -$12,897 |
| 1-hour | edge_vhigh | +$20.16 | 100% | 3 | -$11,515 |

### Critical Parameters

| Parameter | Optimal (5m) | Optimal (1h) | Impact |
|-----------|--------------|--------------|--------|
| entry_threshold | 2.0 | 2.5+ | +++High |
| edge_hurdle | 0.006 | 0.012 | +++High |
| fee_rate | 0.0 (maker) | 0.0002 (maker) | +++High |
| stop_loss | 0.003 | 0.008 | ++Medium |
| take_profit | 0.012 | 0.015 | ++Medium |
| position_size | 0.002 | 0.001 | +Low |

### Risk Metrics

| Metric | 5-min | 1-hour | Target |
|--------|-------|--------|--------|
| Max Drawdown | -0.83% | -0.40% | < 2% |
| Win Rate | 66.7% | 53.6% | > 50% |
| Sharpe Proxy | 1.42 | 4.29 | > 1.0 |
| Friction Ratio | 0% | 18% | < 30% |

---

## Qualitative Assessment

### Strengths

1. **Trend Awareness**: Strategy correctly identifies and follows trends via EMA crossover
2. **Regime Adaptation**: Volatility-based regime switching (momentum vs mean-reversion)
3. **Risk Bounds**: Position limits, stop losses, and circuit breakers
4. **Ethical Framework**: Three Poisons guards prevent harmful trading patterns

### Weaknesses

1. **Friction Sensitivity**: Taker fees turn profitable strategies into losers
2. **Overtrading**: Default parameters trade too frequently on longer timeframes
3. **Parameter Fragility**: Small parameter changes have large outcome effects
4. **Equity Calculation**: Mark-to-market during holds creates volatility

### Opportunities

1. **Maker Order Migration**: Using limit orders could capture +$7-20 per strategy
2. **Dynamic Timeframe Selection**: Adapt parameters based on detected volatility regime
3. **Multi-Strategy Ensemble**: Combine tf_short and tf_long for different conditions
4. **Walk-Forward Optimization**: Currently only in-sample testing

### Threats

1. **Regime Change**: Bear market parameters may fail in bull/sideways markets
2. **Liquidity Events**: Slippage model may underestimate flash crash costs
3. **API Latency**: Backtest assumes instant fills, live may differ
4. **Overfitting Risk**: 29 hypotheses on 1000 bars may be curve-fitting

---

## Recommendations

### Immediate (Pre-Live)

1. **Implement maker-only mode**: Target limit order fills at or better than mid
2. **Add timeframe parameter**: Auto-adjust entry_threshold based on candle_granularity
3. **Increase edge_hurdle default**: From 0.003 to 0.006 minimum

### Short-Term (First Week Live)

1. **Paper trade first**: Run with 0.0001 BTC positions for validation
2. **Monitor friction ratio**: Alert if friction > 50% of gross PnL
3. **Track regime accuracy**: Log whether trend detection matches market behavior

### Long-Term (Optimization)

1. **Walk-forward validation**: Split data 70/30, optimize on first, validate on second
2. **Cross-asset testing**: Test on ETH, SOL for generalization
3. **Monte Carlo simulation**: Bootstrap confidence intervals on key metrics

---

## Test Coverage Assessment

| Module | Unit Tests | Integration | Property |
|--------|------------|-------------|----------|
| state.rs | 25+ | - | - |
| strategy.rs | 10+ | - | - |
| backtest.rs | 3 | 1 | - |
| risk.rs | - | - | - |
| Total | 124 | 1 | 0 |

**Gap**: Risk engine needs unit tests, property-based tests for invariants.

---

## Approval Status

- [ ] Code Review: Pending
- [x] Backtest Validation: Pass (conditional)
- [ ] Risk Review: Pending
- [ ] Ethics Review: Pending
- [ ] Live Deployment: Not Ready

**Overall Assessment**: CONDITIONAL PASS - profitable under specific conditions, needs parameter hardening before live.
