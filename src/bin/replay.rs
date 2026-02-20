use std::collections::HashSet;
use std::io::{self, BufRead};

use arbitragefx::state::Fill;
use arbitragefx::strategy::PortfolioState;
use arbitragefx::verify::invariants::{
    apply_fill_idempotent, assert_equity_consistency, assert_portfolio_invariants,
};
use arbitragefx::verify::order_sm::{apply_event, Event, Order, OrderState};

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum InputEvent {
    Submit,
    Ack {
        order_id: String,
    },
    Fill {
        fill_id: String,
        qty: f64,
        price: f64,
        ts: u64,
    },
    CancelRequest,
    CancelAck,
    Reject {
        reason: String,
    },
    Timeout,
}

fn main() {
    let stdin = io::stdin();
    let mut order = Order::new("client-1".to_string(), 1.0);
    let mut portfolio = PortfolioState {
        cash: 1000.0,
        position: 0.0,
        entry_price: 0.0,
        equity: 1000.0,
    };
    let mut seen = HashSet::new();

    for line in stdin.lock().lines().flatten() {
        if line.trim().is_empty() {
            continue;
        }
        let evt: InputEvent = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("bad event json: {}", err);
                continue;
            }
        };

        let mut mark_price = None;
        let event = match evt {
            InputEvent::Submit => Event::Submit,
            InputEvent::Ack { order_id } => Event::Ack { order_id },
            InputEvent::Fill {
                fill_id,
                qty,
                price,
                ts,
            } => {
                let fill = Fill {
                    price,
                    qty,
                    fee: 0.0,
                    ts,
                };
                let _ = apply_fill_idempotent(&mut portfolio, fill, &mut seen, &fill_id);
                mark_price = Some(price);
                Event::Fill {
                    fill_id,
                    qty,
                    price,
                }
            }
            InputEvent::CancelRequest => Event::CancelRequest,
            InputEvent::CancelAck => Event::CancelAck,
            InputEvent::Reject { reason } => Event::Reject { reason },
            InputEvent::Timeout => Event::Timeout,
        };

        let _ = apply_event(&mut order, event);
        let _ = assert_portfolio_invariants(&portfolio);
        if let Some(p) = mark_price {
            let _ = assert_equity_consistency(&portfolio, p, 1e-6);
        }
    }

    let state = match order.state {
        OrderState::New => "New",
        OrderState::Submitted => "Submitted",
        OrderState::Acked => "Acked",
        OrderState::PartiallyFilled => "PartiallyFilled",
        OrderState::Filled => "Filled",
        OrderState::Canceled => "Canceled",
        OrderState::Rejected => "Rejected",
    };
    println!(
        "order_state={state} filled_qty={} equity={}",
        order.filled_qty, portfolio.equity
    );
}
