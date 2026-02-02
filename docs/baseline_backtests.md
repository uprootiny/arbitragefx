# Baseline Backtests

## Environment
- Used existing binaries in `target/release/` due to build failures (no network + cross-device link errors).
- Data: `data/btc_sim.csv`.

## 1) Backtest (churn set)
Command:
```
./target/release/backtest data/btc_sim.csv
```
Output:
```
baseline=buy_hold pnl=750.0000
baseline=no_trade pnl=0.0000
strategy=churn-0 pnl=0.0000 friction=10.5985 friction_only_pnl=-10.5985 dd=-21.2020
strategy=churn-1 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-2 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-3 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-4 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-5 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-6 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-7 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-8 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-9 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-10 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
strategy=churn-11 pnl=0.0000 friction=0.0000 friction_only_pnl=-0.0000 dd=0.0000
pnl_total=0.0000 max_drawdown=-21.2020
```

Observations:
- Only `churn-0` traded enough to incur friction.
- All other variants produced zero PnL and zero friction, indicating no trades.
- Baseline buy/hold > 0, no-trade is 0, so the strategy set produced no edge on this dataset.

## 2) Event Backtest (phase1)
Command:
```
./target/release/event_backtest data/btc_sim.csv
```
Output:
```
event,pnl,count,avg_pnl
```

Observations:
- No events detected with current thresholds on this dataset.

## Build Notes
- `cargo run` fails due to lack of crates.io access and cross-device link errors when writing to `/tmp`.
- Existing release binaries are usable for baseline runs.
