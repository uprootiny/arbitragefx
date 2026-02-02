# Issue Triage

**Last Updated**: 2026-02-01
**Status**: Active Development

---

## Open Issues

### ISSUE-001: Friction Costs Dominate Returns
**Status**: MITIGATED
**Priority**: P0

Taker fees cause 113-467% drag. **Mitigation**: Use maker orders (limit orders).

### ISSUE-002: Overtrading on Longer Timeframes
**Status**: MITIGATED
**Priority**: P1

507/1000 bars have trades on 1h. **Mitigation**: Use `edge_vhigh` (0.012 hurdle) or `tf_long_*` configs.

### ISSUE-008: Slippage Model May Be Unrealistic
**Status**: ACKNOWLEDGED
**Priority**: P2

Slippage formula not calibrated to live data. Need fill data from live trading.

### ISSUE-011: Missing Property-Based Tests
**Status**: ACKNOWLEDGED
**Priority**: P3

No QuickCheck/proptest. Lower priority.

---

## Resolved Issues

### ISSUE-R01: Cash Flow Bug ✓
Sells now correctly add to cash.

### ISSUE-R02: Long Bias in SimpleMomentum ✓
Added trend filter, balanced signal distribution.

### ISSUE-R03: Equity Not Mark-to-Market ✓
Added `MetricsEngine::update_with_price()`.

### ISSUE-R04: Kelly Sizing Not Implemented ✓
Added `kelly_size()`, `expectancy()`, `risk_of_ruin()`, `limit_fill_probability()`.

### ISSUE-R05: Expectancy Not Tracked ✓
Added `MetricsState::expectancy()` and `record_trade()`.

### ISSUE-R06: Walk-Forward Validation ✓
Created train/test splits, validated across timeframes.

### ISSUE-R07: Multi-Asset Validation ✓
Added ETH, SOL data. Strategies break even cross-asset.

### ISSUE-003: Position Sizing Paradox ✓
**Explained**: Smaller positions have higher friction-to-profit ratio.

### ISSUE-004: Max Drawdown Not Tracked ✓
Fixed in diagnose.rs. Now shows -1.76% correctly.

### ISSUE-005: Guard Stats Empty ✓
Guards now logged when triggered.

### ISSUE-006: Strategy ID Fields Unused ✓
Added `#[allow(dead_code)]`.

### ISSUE-009: No Multi-Asset Support ✓
ETH and SOL data fetched and validated.

### ISSUE-010: Unused Fields ✓
Added `#[allow(dead_code)]` to API response structs.

---

## Summary

| Priority | Open | Resolved |
|----------|------|----------|
| P0 | 1 (mitigated) | 3 |
| P1 | 1 (mitigated) | 4 |
| P2 | 1 | 3 |
| P3 | 1 | 4 |
| **Total** | **4** | **14** |

---

## Metrics

- **Tests**: 130 passing
- **Hypotheses**: 33
- **Profitable configs (5m)**: 9/33 (27%)
- **Profitable configs (1h)**: 5/33 (15%)
- **Cross-asset**: Break-even on ETH, SOL
- **Max drawdown tracking**: Working (-1.76%)
- **Expectancy tracking**: Working (+$0.038/trade)
