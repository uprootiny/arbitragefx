# ArbitrageFX

Cryptocurrency backtesting workbench with honest friction accounting
and hypothesis-driven strategy evaluation.

## Quick Start

```bash
# Run backtest on real data
cargo run --release --bin backtest -- data/btc_real_1h.csv

# Run all tests (333 tests)
cargo test

# Run smoke tests only (13 tests on real data)
cargo test --test smoke

# Fetch fresh data from Binance
./scripts/fetch_data.sh BTCUSDT 1h 1000

# Run full pipeline: validate → backtest all regimes → report
./scripts/pipeline.sh

# Shell-level smoke tests
./scripts/smoke.sh
```

## What This Does

Runs 12 momentum strategy variants against historical BTC/USDT candle data
with realistic execution simulation:

- **Slippage** — volume and volatility-scaled price impact
- **Fees** — maker/taker fee models
- **Latency** — deterministic jitter (xorshift, fully replayable)
- **Partial fills** — configurable fill ratios
- **Risk guards** — drawdown limits, exposure caps, loss cooldowns

## What We've Learned

From 60 strategy-regime runs across 5 real market periods:

| Finding | Evidence |
|---------|----------|
| Friction dominates raw alpha in ~90% of cases | 5 datasets |
| Position sizing keeps drawdown under 2% (4/5 regimes) | 5 datasets |
| Trade frequency is the #1 friction driver | Correlation >0.95 |
| The system preserves capital (DD 0.82% vs market -24%) | 2 bear datasets |
| No strategy consistently beats doing nothing | 5 datasets |

Full evidence in [`hypothesis_ledger.edn`](hypothesis_ledger.edn).

## Data

| File | Regime | Candles | Move |
|------|--------|---------|------|
| `data/btc_real_1h.csv` | Strong Bear | 1000 | -27% |
| `data/btc_bull_1h.csv` | Strong Bull | 2209 | +49% |
| `data/btc_range_1h.csv` | Ranging | 2209 | +1% |
| `data/btc_bear2_1h.csv` | Mild Bear | 1465 | -2% |

All from Binance public klines API. SHA256-hashed for provenance.

## Project Structure

```
src/
├── backtest.rs      # Core engine: run_backtest(), run_backtest_full()
├── strategy.rs      # Types: Candle, Action, Strategy trait
├── state.rs         # Config, MarketState, SimpleMomentum
├── risk.rs          # RiskEngine: Kelly, exposure, cooldown
├── engine/          # Event-sourced architecture (experimental)
├── strategies.rs    # 7 composable strategy types
├── signals.rs       # 16 pure signal functions
├── indicators.rs    # EMA, RSI, MACD, Bollinger, ATR
├── exchange/        # Binance/Kraken API clients
└── bin/             # 23 binaries (backtest, sweep, research, etc.)

tests/
├── smoke.rs                # 13 end-to-end tests on real data
├── backtest_validation.rs  # 26 execution model invariant tests
├── drift_integration.rs    # 3 regime-change response tests
└── data_quality.rs         # 3 CSV schema/quality tests

scripts/
├── smoke.sh         # Shell-level smoke tests
├── fetch_data.sh    # Pull candles from Binance API
└── pipeline.sh      # Full fetch → validate → backtest → report
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full module map and system diagram.

## Configuration

All 54 parameters via environment variables. Key ones:

```bash
EXEC_MODE=market     # instant | market | limit | realistic
FEE_RATE=0.001       # 0.1% taker fee
SLIP_K=0.0008        # Slippage coefficient
MAX_POS_PCT=0.05     # 5% max position size
STOP_LOSS=0.004      # 0.4% stop loss
TAKE_PROFIT=0.006    # 0.6% take profit
```

## Testing

```bash
cargo test                    # All 333 tests
cargo test --test smoke       # 13 smoke tests on real data
cargo test --test backtest_validation  # 26 execution invariants
./scripts/smoke.sh            # 8 shell smoke tests
```

## License

MIT
