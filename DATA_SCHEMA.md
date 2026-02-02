# Data Schema (Engineering-Grade)

All data is **minute candles** plus auxiliary series. CSV is the minimal
transport; keep timestamps in epoch seconds.

## Primary Table: `candles`

```
ts,open,high,low,close,volume,funding,borrow,liq,depeg,oi
```

Field definitions:
- `ts` (u64): epoch seconds
- `open/high/low/close` (f64)
- `volume` (f64)
- `funding` (f64): perp funding rate for interval
- `borrow` (f64): borrow rate (spot or margin)
- `liq` (f64): liquidation score (normalized spike index)
- `depeg` (f64): stablecoin deviation from peg (e.g., -0.003 = -0.3%)
- `oi` (f64): open interest

## Optional Tables

### `funding_history`
- `ts, symbol, funding_rate`

### `borrow_history`
- `ts, asset, borrow_rate`

### `liq_events`
- `ts, symbol, liq_score, liq_volume`

### `stable_depeg`
- `ts, symbol, depeg`

### `oi_series`
- `ts, symbol, oi`

## Feature Pipeline Outputs (per candle)

- `funding_p95`: rolling 95th percentile
- `funding_flip`: sign change
- `oi_change`: ΔOI / OI
- `price_velocity`: (p_t - p_{t-1}) / p_{t-1}
- `vol_ratio`: short_vol / long_vol
- `vol_compress`: short_vol < p10(short_vol)
- `liq_score`: passthrough
- `stable_depeg`: passthrough
