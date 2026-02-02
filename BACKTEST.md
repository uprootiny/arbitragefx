# Backtest Harness (API-Only)

## Input CSV schema

```
ts,open,high,low,close,volume,funding,borrow,liq,depeg,oi
```

All values are numeric. `ts` is epoch seconds.

## Run

```
cargo run --bin backtest path/to/data.csv
```

## Event backtest

```
cargo run --bin event_backtest path/to/data.csv
```

## Output

```
strategy=churn-0 pnl=... friction=... dd=...
...
pnl_total=... max_drawdown=...
```

Baselines:
- `buy_hold` (first close → last close)
- `no_trade` (0.0)
- `friction_only_pnl` per strategy (=-friction)

## Notes
- This uses the same strategy triggers wired in `TRIGGERS.md`.
- Execution physics modeled: latency, partial fills, slippage, fees.
- Feature pipeline uses funding percentile, OI change, price velocity, vol ratios.

## Execution knobs (env)
- `SLIP_K` (default 0.0008)
- `FEE_RATE` (default 0.001)
- `LAT_MIN` (default 2)
- `LAT_MAX` (default 8)
- `FILL_RATIO` (default 0.5)
