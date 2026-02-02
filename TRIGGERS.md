# Ground-Level Triggers (Load-Bearing Defaults)

This is the minimal, **executable** trigger set wired into the strategy.
All are O(1), API-only, and safe for non-HFT infra.

## 1) Funding Carry
- Condition: `|funding_rate| > FUNDING_HIGH`
- Additional: `borrow_rate < |funding_rate| - FUNDING_SPREAD`
- Action: trade **against** funding pressure (positive funding → short, negative → long)

## 2) Liquidation Cascade Momentum
- Condition: `liquidation_score > LIQ_SCORE_TH`
- Direction: follow `z_momentum`

## 3) Stablecoin Depeg Snapback
- Condition: `|stable_depeg| > DEPEG_TH`
- Direction: fade the depeg (below peg → buy, above peg → sell)

## 4) Volatility Regime Switch
- Compute: `vol_ratio = vol / vol_mean`
- If `vol_ratio < VOL_LOW`: follow momentum (`z_momentum`)
- If `vol_ratio > VOL_HIGH`: mean revert on stretch (`z_stretch`)

## 5) Edge Hurdle
- Require `expected_edge = |score| * EDGE_SCALE` to exceed `EDGE_HURDLE`
- `score = 1.0*z_momentum + 0.5*z_vol + 0.7*z_volume_spike - 0.8*z_stretch`

## Exit Logic
- Take-profit: `TAKE_PROFIT` (default 0.6%)
- Stop-loss: `STOP_LOSS` (default 0.4%)
- Time stop: `TIME_STOP * CANDLE_SECS`
- If `|score| < EXIT_TH` → exit

All thresholds are configurable via environment variables.
