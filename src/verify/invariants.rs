use crate::state::Fill;
use crate::strategy::PortfolioState;

#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub msg: String,
}

pub fn assert_order_invariants(qty: f64, filled_qty: f64) -> Result<(), InvariantViolation> {
    if filled_qty < -1e-9 {
        return Err(InvariantViolation {
            msg: "filled_qty negative".to_string(),
        });
    }
    if filled_qty - qty > 1e-9 {
        return Err(InvariantViolation {
            msg: "filled_qty exceeds order qty".to_string(),
        });
    }
    Ok(())
}

pub fn apply_fill_idempotent(
    portfolio: &mut PortfolioState,
    fill: Fill,
    seen: &mut std::collections::HashSet<String>,
    fill_id: &str,
) -> Result<f64, InvariantViolation> {
    if seen.contains(fill_id) {
        return Ok(0.0);
    }
    seen.insert(fill_id.to_string());
    Ok(portfolio.apply_fill(fill))
}

pub fn assert_portfolio_invariants(portfolio: &PortfolioState) -> Result<(), InvariantViolation> {
    if portfolio.cash.is_nan() || portfolio.equity.is_nan() {
        return Err(InvariantViolation {
            msg: "NaN in portfolio state".to_string(),
        });
    }
    Ok(())
}

pub fn assert_equity_consistency(
    portfolio: &PortfolioState,
    mark_price: f64,
    tolerance: f64,
) -> Result<(), InvariantViolation> {
    let expected = portfolio.cash + portfolio.position * mark_price;
    if (portfolio.equity - expected).abs() > tolerance {
        return Err(InvariantViolation {
            msg: "equity not consistent with cash + position".to_string(),
        });
    }
    Ok(())
}
