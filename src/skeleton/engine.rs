use super::exec::{Execution, Fill};
use super::log::Logger;
use super::state::{Candle, EngineState};
use super::strategy::{Action, Strategy};
use super::wal::Wal;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sleep_secs: u64,
    pub max_position: f64,
}

pub fn run_loop<S, E>(
    state: &mut EngineState,
    strat: &mut S,
    exec: &mut E,
    wal: &mut Wal,
    logger: &mut Logger,
    cfg: &EngineConfig,
    candles: impl Iterator<Item = Candle>,
) where
    S: Strategy,
    E: Execution,
{
    for candle in candles {
        state.market.update(candle);
        let price = state.market.last.c;
        let action = if state.risk.halted {
            Action::Hold
        } else {
            strat.decide(state)
        };
        let action = risk_guard(action, state, cfg);

        if let Action::Hold = action {
            logger.log("strategy.hold", "{}");
            continue;
        }

        let fill = exec.place(price, action);
        wal.append(&format!(
            "{{\"ts\":{},\"intent\":\"{}\",\"qty\":{},\"price\":{}}}",
            now_ts(),
            strat.id(),
            fill.qty,
            fill.price
        ));
        apply_fill(state, fill);
        logger.log("fill.applied", "{}");
    }
}

fn apply_fill(state: &mut EngineState, fill: Fill) {
    state.portfolio.apply_fill(fill.price, fill.qty, fill.fee);
    state.risk.last_trade_ts = now_ts();
}

fn risk_guard(action: Action, state: &EngineState, cfg: &EngineConfig) -> Action {
    match action {
        Action::Buy { qty } | Action::Sell { qty } => {
            let price = state.market.last.c;
            let exposure = (state.portfolio.position + qty) * price;
            if exposure.abs() > cfg.max_position {
                return Action::Hold;
            }
            action
        }
        Action::Close | Action::Hold => action,
    }
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
