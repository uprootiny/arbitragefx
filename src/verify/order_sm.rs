use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderState {
    New,
    Submitted,
    Acked,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub client_id: String,
    pub order_id: Option<String>,
    pub state: OrderState,
    pub qty: f64,
    pub filled_qty: f64,
    pub seen_fills: HashSet<String>,
}

impl Order {
    pub fn new(client_id: String, qty: f64) -> Self {
        Self {
            client_id,
            order_id: None,
            state: OrderState::New,
            qty,
            filled_qty: 0.0,
            seen_fills: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Submit,
    Ack {
        order_id: String,
    },
    Fill {
        fill_id: String,
        qty: f64,
        price: f64,
    },
    CancelRequest,
    CancelAck,
    Reject {
        reason: String,
    },
    Timeout,
}

#[derive(Debug, Clone)]
pub struct TransitionError {
    pub msg: String,
}

pub fn apply_event(order: &mut Order, event: Event) -> Result<(), TransitionError> {
    match (&order.state, event) {
        (OrderState::New, Event::Submit) => {
            order.state = OrderState::Submitted;
            Ok(())
        }
        (OrderState::Submitted, Event::Ack { order_id }) => {
            order.order_id = Some(order_id);
            order.state = OrderState::Acked;
            Ok(())
        }
        (OrderState::Submitted, Event::Reject { .. }) => {
            order.state = OrderState::Rejected;
            Ok(())
        }
        (OrderState::Acked, Event::Fill { fill_id, qty, .. })
        | (OrderState::PartiallyFilled, Event::Fill { fill_id, qty, .. }) => {
            if order.seen_fills.contains(&fill_id) {
                return Ok(());
            }
            order.seen_fills.insert(fill_id);
            order.filled_qty += qty;
            if order.filled_qty + 1e-9 >= order.qty {
                order.state = OrderState::Filled;
            } else {
                order.state = OrderState::PartiallyFilled;
            }
            Ok(())
        }
        (OrderState::Acked, Event::CancelRequest) => {
            order.state = OrderState::Canceled;
            Ok(())
        }
        (OrderState::PartiallyFilled, Event::CancelRequest) => {
            order.state = OrderState::Canceled;
            Ok(())
        }
        (OrderState::Canceled, Event::CancelAck) => Ok(()),
        (OrderState::Submitted, Event::Timeout) => {
            order.state = OrderState::Canceled;
            Ok(())
        }
        (OrderState::Acked, Event::Timeout) => {
            order.state = OrderState::Canceled;
            Ok(())
        }
        (OrderState::Rejected, _) => Ok(()),
        (OrderState::Filled, _) => Ok(()),
        (_, Event::Reject { .. }) => {
            order.state = OrderState::Rejected;
            Ok(())
        }
        (_, Event::Ack { .. }) | (_, Event::Submit) => Err(TransitionError {
            msg: "invalid lifecycle transition".to_string(),
        }),
        _ => Ok(()),
    }
}
