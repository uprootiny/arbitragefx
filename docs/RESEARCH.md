# Hypothesis-Driven Strategy Research

This document tracks our systematic approach to trading strategy development.

## Methodology

1. **Formulate** testable hypotheses about market behavior
2. **Test** each hypothesis with backtests across market regimes
3. **Record** evidence in the ledger
4. **Refine** or reject hypotheses based on accumulated evidence
5. **Combine** supported hypotheses into composite strategies

## Current Findings (StrongBear regime, -28.4%)

### Supported Hypotheses

| ID | Hypothesis | PnL | Sharpe | Win% |
|----|------------|-----|--------|------|
| H006 | Short-selling in bear markets outperforms | +17,075 | 1.96 | 38% |
| H004 | Trend filtering improves trade quality | +1,302 | 0.62 | 44% |
| H008 | Volatility breakouts are tradeable | +815 | 0.45 | 40% |

### Key Insights

1. **Win rate is not everything**: H006 had only 38% win rate but exceptional returns because winners were much larger than losers.

2. **Trend alignment matters**: H004 showed that filtering trades to align with EMA trend significantly improved Sharpe ratio vs unfiltered.

3. **Volatility-based sizing backfired**: H003 actually lost money, suggesting that in trending markets, reducing position size on vol spikes may miss the best moves.

4. **Mean reversion struggles in trends**: H002/H005 (RSI/stretch reversion) were positive but underperformed trend-following in strong directional markets.

### Refuted in Bear Markets

| ID | Hypothesis | PnL | Sharpe | Issue |
|----|------------|-----|--------|-------|
| H003 | Vol scaling improves returns | -1,075 | 0.38 | Lost money |
| H001 | Momentum predicts continuation | +73 | -0.12 | Negative Sharpe |
| H007 | Confluence improves quality | +780 | 0.36 | Below threshold |
| H002 | Mean reversion works | +967 | 0.39 | Below threshold |
| H005 | RSI extremes predict reversals | +518 | 0.24 | Below threshold |

## Next Steps

1. **Test in Bull Markets**: Need BTC data from 2020-2021 bull run
2. **Test in Ranging Markets**: Find sideways periods for mean reversion tests
3. **Composite Strategy**: Combine H006 (bear) with H001 (bull) using regime detection
4. **Refine H003**: Volatility sizing may need different implementation

## Running Experiments

```bash
# List hypotheses
./target/release/research_lab hypotheses

# Test single hypothesis
./target/release/research_lab test H006_bear_short data/btc_1h_180d.csv

# Test all hypotheses
./target/release/research_lab test-all data/btc_1h_180d.csv

# View evidence
./target/release/research_lab evidence

# Get suggestions
./target/release/research_lab suggest
```

## Evidence Ledger

Stored in `data/hypothesis_ledger.json`. Contains:
- All hypotheses with success criteria
- Evidence from each backtest
- Regime classification
- Support/refute determination

## Strategy Recombination Ideas

Based on evidence, promising combinations:

1. **Adaptive Trend-Following**:
   - Use H004 (trend filter) + H006 (short bias) in bear
   - Use H004 (trend filter) + H001 (momentum) in bull
   - Regime detection via 50-day price change

2. **Breakout + Trend**:
   - H008 (vol breakout) for entries
   - H004 (trend filter) for direction
   - Should catch major moves

3. **Multi-Timeframe Confluence**:
   - Higher timeframe: trend direction (H004)
   - Lower timeframe: entry timing (H008 breakout)
   - Position sizing: ATR-based (not vol-scaled)
