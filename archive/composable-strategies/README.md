# composable-strategies (archived)

A composable strategy framework where strategies are built from
signal + filter + sizing building blocks.

## What's here

- **strategies.rs** (808 lines) — Strategy implementations: MomentumStrategy,
  MeanReversionStrategy, FundingCarryStrategy, LiquidationCascadeStrategy,
  VolatilityBreakoutStrategy, MultiFactorStrategy, AdaptiveStrategy.
  Each combines entry signals, exit rules, filters, and position sizing.
- **strategy_lab.rs** (692 lines) — Interactive research tool for testing
  composable strategies against historical data.
- **strategy_sweep.rs** (454 lines) — Parameter sweep tool for composable
  strategies.

## Why archived

The active pipeline uses 12 `SimpleMomentum` variants (churn-1 through churn-12)
defined in `state.rs`. These are simpler but thoroughly tested with walk-forward
validation and Bonferroni correction. Rewriting to use the composable layer
would require significant effort for uncertain benefit.

## Spin-off potential

This is a well-designed strategy composition framework. With indicators.rs
and signals.rs, it forms a complete "build your own strategy" toolkit.

## Quality

Compiles. Has unit tests. Clean Strategy trait implementation. The framework
design is sound — the issue was never quality, it was that SimpleMomentum
served the immediate need.
