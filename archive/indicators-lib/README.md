# indicators-lib (archived)

Signal building blocks and position sizing that were part of the composable
strategy layer. The core indicator module (`indicators.rs`) has been wired
back into the active codebase.

## What's here

- **signals.rs** (516 lines) — Pure signal functions: `MarketView → Signal`.
  Algebraic Signal type with combine/blend. Momentum, mean-reversion, trend,
  funding carry, liquidation cascade signals.
- **filters.rs** (413 lines) — Trade filters: volatility, trend, time-of-day,
  cooldown, drawdown circuit breaker.
- **sizing.rs** (371 lines) — Position sizing: fixed, volatility-inverse, Kelly
  criterion, regime-adaptive.

## Why archived

The active backtest pipeline uses `SimpleMomentum` in `state.rs` which inlines
its own signal logic. These modules were designed for a composable strategy
framework (`strategies.rs`) that was never wired into the production path.

## Spin-off potential

signals.rs + filters.rs + sizing.rs form a clean composable trading signal
library. Combined with indicators.rs (now in active codebase), this could
become a standalone `rustquant-signals` crate.

## Quality

All modules compile. signals.rs has test coverage. Pure functions with no
side effects. Clean interfaces.
