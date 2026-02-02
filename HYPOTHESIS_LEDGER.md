# Hypothesis Ledger

A structured ledger of hypotheses, tests, evidence, and decisions.

## Schema
Each entry uses this template:

```
- id: HYP-YYYYMMDD-NN
  title: <short claim>
  status: proposed | testing | supported | refuted | archived
  rationale: <why we believe this might be true>
  scope: <market/regime/timeframe>
  constraints: <limits / applicability>
  tests:
    - id: TEST-...
      method: <backtest/sim/live paper>
      dataset: <data source + range>
      metrics: <sharpe, drawdown, etc>
      result: <summary>
      artifacts: <paths/links>
  evidence:
    - id: EVID-...
      type: plot | log | report | statistic
      summary: <what it shows>
      strength: low | medium | high
  decision:
    date: <YYYY-MM-DD>
    outcome: keep | refine | drop | defer
    notes: <why>
```

## Entries

- id: HYP-20260202-01
  title: Mean-reversion improves stability in high-volatility regimes
  status: proposed
  rationale: Volatility spikes often overshoot; reverting signals may stabilize drawdown.
  scope: BTC/ETH, 5m–1h, high-vol regimes
  constraints: avoid low-liquidity windows
  tests: []
  evidence: []
  decision:
    date: 2026-02-02
    outcome: defer
    notes: awaiting baseline backtest segmentation
