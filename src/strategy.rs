// Strategy API + state layout for rolling backtests.

#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub ts: u64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MarketAux {
    pub funding_rate: f64,
    pub borrow_rate: f64,
    pub liquidation_score: f64,
    pub stable_depeg: f64,
    /// Timestamp when this data was fetched (0 = never/missing)
    pub fetch_ts: u64,
    /// Flags indicating which fields have real data vs defaults
    pub has_funding: bool,
    pub has_borrow: bool,
    pub has_liquidations: bool,
    pub has_depeg: bool,
}

/// Requirements for aux data - different strategies need different fields
#[derive(Debug, Clone, Copy, Default)]
pub struct AuxRequirements {
    pub needs_funding: bool,
    pub needs_borrow: bool,
    pub needs_liquidations: bool,
    pub needs_depeg: bool,
}

impl AuxRequirements {
    /// Funding carry strategy needs funding rate
    pub fn for_funding_carry() -> Self {
        Self { needs_funding: true, needs_borrow: true, ..Default::default() }
    }

    /// Event-driven strategy needs liquidation data
    pub fn for_event_driven() -> Self {
        Self { needs_liquidations: true, needs_depeg: true, ..Default::default() }
    }

    /// Mean reversion can work with minimal aux data
    pub fn for_mean_reversion() -> Self {
        Self::default()
    }

    /// Full aux data for comprehensive strategies
    pub fn full() -> Self {
        Self { needs_funding: true, needs_borrow: true, needs_liquidations: true, needs_depeg: true }
    }
}

impl MarketAux {
    /// Check if aux data is stale (older than max_age_secs)
    pub fn is_stale(&self, now_ts: u64, max_age_secs: u64) -> bool {
        if self.fetch_ts == 0 {
            return true;
        }
        now_ts.saturating_sub(self.fetch_ts) > max_age_secs
    }

    /// Check if any required field is missing (legacy: funding OR borrow)
    pub fn is_incomplete(&self) -> bool {
        !self.has_funding && !self.has_borrow
    }

    /// Check against specific requirements
    pub fn meets_requirements(&self, reqs: &AuxRequirements) -> bool {
        (!reqs.needs_funding || self.has_funding) &&
        (!reqs.needs_borrow || self.has_borrow) &&
        (!reqs.needs_liquidations || self.has_liquidations) &&
        (!reqs.needs_depeg || self.has_depeg)
    }

    /// Check if aux data is valid for trading decisions
    pub fn is_valid_for_trading(&self, now_ts: u64, max_age_secs: u64) -> bool {
        !self.is_stale(now_ts, max_age_secs) && !self.is_incomplete()
    }

    /// Check validity with specific requirements
    pub fn is_valid_for_strategy(&self, now_ts: u64, max_age_secs: u64, reqs: &AuxRequirements) -> bool {
        !self.is_stale(now_ts, max_age_secs) && self.meets_requirements(reqs)
    }

    /// Age of data in seconds
    pub fn age_secs(&self, now_ts: u64) -> u64 {
        if self.fetch_ts == 0 { u64::MAX } else { now_ts.saturating_sub(self.fetch_ts) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MarketView<'a> {
    // Read-only pointers into shared state.
    pub symbol: &'a str,
    pub last: Candle,
    pub indicators: IndicatorSnapshot,
    pub aux: MarketAux,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IndicatorSnapshot {
    pub ema_fast: f64,
    pub ema_slow: f64,
    pub vwap: f64,
    pub vol: f64,
    pub vol_mean: f64,
    pub momentum: f64,
    pub volume_spike: f64,
    pub stretch: f64,
    pub z_momentum: f64,
    pub z_vol: f64,
    pub z_volume_spike: f64,
    pub z_stretch: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PortfolioState {
    pub cash: f64,
    pub position: f64,
    pub entry_price: f64,
    pub equity: f64,
}

impl PortfolioState {
    pub fn apply_fill(&mut self, fill: crate::state::Fill) -> f64 {
        if fill.qty == 0.0 {
            return 0.0;
        }
        let mut realized = 0.0;
        let prev_pos = self.position;
        let new_pos = prev_pos + fill.qty;

        // Realized PnL on the portion that closes existing position
        if prev_pos != 0.0 && prev_pos.signum() != fill.qty.signum() {
            let close_qty = prev_pos.abs().min(fill.qty.abs());
            let dir = if prev_pos > 0.0 { 1.0 } else { -1.0 };
            realized = (fill.price - self.entry_price) * close_qty * dir;
        }

        let cost = fill.price * fill.qty.abs();
        self.cash -= cost + fill.fee;
        self.position = new_pos;

        // Update entry price based on add/reduce/flip
        if prev_pos == 0.0 {
            self.entry_price = fill.price;
        } else if prev_pos.signum() == new_pos.signum() {
            if new_pos.abs() > prev_pos.abs() {
                // Increasing position: use weighted average entry
                let total = prev_pos.abs() + fill.qty.abs();
                if total > 0.0 {
                    self.entry_price =
                        (self.entry_price * prev_pos.abs() + fill.price * fill.qty.abs()) / total;
                }
            }
            // Reducing position: keep entry price
        } else if new_pos != 0.0 {
            // Flipped position: new entry is fill price
            self.entry_price = fill.price;
        }

        self.equity = self.cash + (self.position * fill.price);
        realized
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StrategyState {
    // Per-instance mutable state owned by the strategy.
    pub portfolio: PortfolioState,
    pub metrics: MetricsState,
    pub last_trade_ts: u64,
    pub last_loss_ts: u64,
    pub trading_halted: bool,
    pub trades_today: u32,
    pub trade_day: u64,
    pub order_seq: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsState {
    pub wins: u64,
    pub losses: u64,
    pub pnl: f64,
    pub equity_peak: f64,
    pub max_drawdown: f64,
    // Welford running stats for returns.
    pub n: u64,
    pub mean: f64,
    pub m2: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Hold,
    Buy { qty: f64 },
    Sell { qty: f64 },
    Close,
}

#[derive(Debug, Clone, Copy)]
pub struct Decision {
    pub action: Action,
    pub score: f64,
    pub reason: &'static str,
}

pub trait Strategy {
    fn id(&self) -> &'static str;

    fn update(&mut self, market: MarketView, state: &mut StrategyState) -> Action;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_aux(
        fetch_ts: u64,
        has_funding: bool,
        has_borrow: bool,
        has_liquidations: bool,
        has_depeg: bool,
    ) -> MarketAux {
        MarketAux {
            funding_rate: if has_funding { 0.0001 } else { 0.0 },
            borrow_rate: if has_borrow { 0.00005 } else { 0.0 },
            liquidation_score: if has_liquidations { 1.5 } else { 0.0 },
            stable_depeg: if has_depeg { -0.001 } else { 0.0 },
            fetch_ts,
            has_funding,
            has_borrow,
            has_liquidations,
            has_depeg,
        }
    }

    #[test]
    fn test_stale_detection() {
        let aux = make_aux(1000, true, true, false, false);

        // Fresh data (now=1050, max_age=60)
        assert!(!aux.is_stale(1050, 60));

        // Stale data (now=1100, max_age=60)
        assert!(aux.is_stale(1100, 60));

        // Exactly at boundary
        assert!(!aux.is_stale(1060, 60));
        assert!(aux.is_stale(1061, 60));
    }

    #[test]
    fn test_never_fetched_is_stale() {
        let aux = make_aux(0, true, true, false, false);
        assert!(aux.is_stale(1000, 60));
    }

    #[test]
    fn test_incomplete_detection() {
        // Has neither → incomplete
        let aux1 = make_aux(1000, false, false, false, false);
        assert!(aux1.is_incomplete());

        // Has funding → complete
        let aux2 = make_aux(1000, true, false, false, false);
        assert!(!aux2.is_incomplete());

        // Has borrow → complete
        let aux3 = make_aux(1000, false, true, false, false);
        assert!(!aux3.is_incomplete());

        // Has both → complete
        let aux4 = make_aux(1000, true, true, false, false);
        assert!(!aux4.is_incomplete());
    }

    #[test]
    fn test_requirements_funding_carry() {
        let reqs = AuxRequirements::for_funding_carry();

        // Missing funding → fails
        let aux1 = make_aux(1000, false, true, true, true);
        assert!(!aux1.meets_requirements(&reqs));

        // Missing borrow → fails
        let aux2 = make_aux(1000, true, false, true, true);
        assert!(!aux2.meets_requirements(&reqs));

        // Has both → passes
        let aux3 = make_aux(1000, true, true, false, false);
        assert!(aux3.meets_requirements(&reqs));
    }

    #[test]
    fn test_requirements_event_driven() {
        let reqs = AuxRequirements::for_event_driven();

        // Missing liquidations → fails
        let aux1 = make_aux(1000, true, true, false, true);
        assert!(!aux1.meets_requirements(&reqs));

        // Missing depeg → fails
        let aux2 = make_aux(1000, true, true, true, false);
        assert!(!aux2.meets_requirements(&reqs));

        // Has both → passes
        let aux3 = make_aux(1000, false, false, true, true);
        assert!(aux3.meets_requirements(&reqs));
    }

    #[test]
    fn test_requirements_mean_reversion() {
        let reqs = AuxRequirements::for_mean_reversion();

        // Mean reversion has no requirements
        let aux = make_aux(1000, false, false, false, false);
        assert!(aux.meets_requirements(&reqs));
    }

    #[test]
    fn test_valid_for_strategy_combined() {
        let reqs = AuxRequirements::for_funding_carry();

        // Fresh + meets reqs → valid
        let aux1 = make_aux(1000, true, true, false, false);
        assert!(aux1.is_valid_for_strategy(1050, 60, &reqs));

        // Fresh + missing reqs → invalid
        let aux2 = make_aux(1000, false, false, false, false);
        assert!(!aux2.is_valid_for_strategy(1050, 60, &reqs));

        // Stale + meets reqs → invalid
        let aux3 = make_aux(1000, true, true, false, false);
        assert!(!aux3.is_valid_for_strategy(1100, 60, &reqs));
    }

    #[test]
    fn test_age_calculation() {
        let aux = make_aux(1000, true, true, false, false);

        assert_eq!(aux.age_secs(1050), 50);
        assert_eq!(aux.age_secs(1000), 0);

        // Never fetched → max age
        let never = make_aux(0, false, false, false, false);
        assert_eq!(never.age_secs(1000), u64::MAX);
    }
}
