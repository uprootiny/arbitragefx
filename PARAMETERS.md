# Parameters (Proposed Defaults + Ranges)

This defines concrete decision thresholds and parameter ranges for the
5–120 minute momentum + volatility-regime model using OHLCV + funding.

## Timeframe
- Candle size: 5m (default), range 1m–15m
- Decision cadence: every candle close
- Persistence: every 5 minutes

## Feature Windows
- EMA fast: 6 candles (default), range 4–10
- EMA slow: 24 candles (default), range 16–48
- Volatility window (Welford): 30 candles (default), range 20–60
- Volume mean window: 30 candles (default), range 20–60
- VWAP window: rolling cumulative within session (default), or 50 candles

## Signal Construction
Let:
- momentum = ema_fast - ema_slow
- vol = rolling_sigma
- volume_spike = current_vol / rolling_mean_vol
- stretch = (price - vwap) / vwap

Score:
score = w1 * z_momentum + w2 * z_vol_regime + w3 * z_volume_spike - w4 * z_stretch

Where z_* are z-scored by rolling sigma of each feature (avoid scale dominance).

## Weights (initial defaults)
- w1 (momentum): 1.0 (range 0.6–1.6)
- w2 (vol_regime): 0.5 (range 0.2–0.8)
- w3 (volume_spike): 0.7 (range 0.3–1.2)
- w4 (stretch): 0.8 (range 0.4–1.2)

## Thresholds
- Entry threshold θ: 1.2 (range 0.8–1.8)
- Exit threshold: 0.4 (range 0.2–0.6) – exit when |score| < exit threshold
- Confirmed breakout (for market orders): |score| > 2.0 (range 1.6–2.6)

## Execution + Cost Guard
- Fees + slippage hurdle: 0.30% (range 0.20–0.45%)
- Expected edge estimate: |score| * edge_scale
- edge_scale: 0.25% per score unit (range 0.15–0.35%)
- Require expected_edge > hurdle before placing any order

## Risk Limits
- Max position size: 5% capital (range 2–8%)
- Max trades/day: 20 (range 8–30)
- Max daily loss: 2% (range 1–3%)
- Cooldown after loss: 10 minutes (range 5–20)
- Volatility spike pause: vol > 2.5 × rolling vol median (range 2.0–3.0)

## Funding Filter (perps only)
- Funding skew threshold: |funding_rate| > 0.01% (range 0.005–0.03%)
- If funding is extreme against position, reduce size by 50% or skip entries

## Position Management
- Take-profit: 0.6% (range 0.4–1.2%)
- Stop-loss: 0.4% (range 0.3–0.8%)
- Time stop: 12 candles (default), range 6–24

## Safe Defaults (Summary)
- EMA(6,24), vol window 30, θ=1.2
- Edge hurdle 0.30%, expected_edge = |score| * 0.25%
- TP/SL: 0.6% / 0.4%, max pos 5%, max loss/day 2%
