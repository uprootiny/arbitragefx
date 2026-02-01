use std::collections::HashMap;

use crate::verify::order_sm::{apply_event, Event, Order, OrderState};

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub orders: HashMap<String, Order>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self { orders: HashMap::new() }
    }

    pub fn ensure(&mut self, client_id: &str, qty: f64) {
        self.orders
            .entry(client_id.to_string())
            .or_insert_with(|| Order::new(client_id.to_string(), qty));
    }

    pub fn apply(&mut self, client_id: &str, event: Event) -> Result<(OrderState, OrderState), String> {
        let order = self
            .orders
            .get_mut(client_id)
            .ok_or_else(|| "unknown order".to_string())?;
        let prev = order.state;
        apply_event(order, event).map_err(|e| e.msg)?;
        Ok((prev, order.state))
    }
}
