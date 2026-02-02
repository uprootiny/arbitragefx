# Hypothesis Space Map

## Axes
- Regime: volatility, liquidity, trendiness
- Horizon: 1m, 5m, 1h, 4h, 1d
- Signal class: mean-reversion, momentum, carry, spread
- Market: BTC, ETH, SOL, cross-venue

## Map
| Regime | Horizon | Signal | Candidate Hypothesis |
| --- | --- | --- | --- |
| High vol | 5m–1h | mean-reversion | Short-term overshoot reverts within N bars |
| Low vol | 1h–4h | momentum | Smooth trends sustain for M bars |
| Liquidity shocks | 1m–5m | spread | Microstructure spreads widen predictably |

## Gaps
- Cross-venue latency effects
- Regime transition detection thresholds
- Slippage sensitivity under stress
