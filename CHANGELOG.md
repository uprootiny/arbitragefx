# Changelog

## [0.1.1] - 2026-02-01

### Added
- **Kelly criterion position sizing** (`src/risk.rs`)
  - `kelly_size()` function with fractional Kelly support
  - `expectancy()` calculation
  - `risk_of_ruin()` estimation
  - `limit_fill_probability()` for adverse selection modeling
  - `RiskEngine::record_trade()` for outcome tracking
  - `RiskEngine::kelly_position_size()` for dynamic sizing

- **Expectancy tracking** (`src/strategy.rs`)
  - `MetricsState.total_win_amount` / `total_loss_amount` fields
  - `MetricsState::expectancy()` method
  - `MetricsState::record_trade()` method

- **Walk-forward validation infrastructure**
  - Train/test splits in `data/splits/`
  - Cross-asset data (ETH, SOL) in `data/`

- **Expanded parameter sweep** (`src/bin/sweep.rs`)
  - 33 hypotheses (up from 13)
  - Timeframe-specific variants (`tf_long_*`, `tf_short_*`)
  - Edge hurdle tests
  - Category-wise analysis output

- **Documentation** (`docs/`)
  - `ASSESSMENT.md` - Quantitative evaluation
  - `DESIGN.md` - System architecture
  - `ISSUES.md` - Issue triage
  - `ARCHITECTURE_SPECULATION.md` - Future directions
  - `CRITIQUE.md` - Adversarial review

### Fixed
- **Cash flow bug** in `PortfolioState::apply_fill()` - sells now correctly add to cash
- **Long bias** in SimpleMomentum strategy - added trend filter
- **Mark-to-market** - equity now updates continuously via `MetricsEngine::update_with_price()`
- **Clone derive** on `skeleton::Logger` and `skeleton::Wal` - removed (BufWriter not Clone)

### Changed
- Score formula rebalanced for trend awareness
- Sweep output now shows per-strategy averages
- Entry threshold defaults adjusted based on timeframe analysis

### Metrics
- Tests: 124 → 130 passing
- Hypotheses tested: 33 across 3 timeframes
- Cross-asset validation: BTC, ETH, SOL
- Walk-forward validation: implemented

---

## [0.1.0] - 2026-01-31

### Initial Release
- Core trading engine
- SimpleMomentum and CarryOpportunistic strategies
- Binance adapter
- WAL-based crash recovery
- Risk engine with position limits
- Backtest infrastructure
