use super::types::{OrderRequest, OrderResponse};

pub trait UnifiedAdapter {
    fn place_order(&mut self, req: OrderRequest) -> Result<OrderResponse, String>;
    fn cancel_order(&mut self, order_id: &str) -> Result<(), String>;
    fn cancel_all(&mut self) -> Result<(), String>;
}

// Stub implementation to make integration explicit.
pub struct NullAdapter;

impl UnifiedAdapter for NullAdapter {
    fn place_order(&mut self, req: OrderRequest) -> Result<OrderResponse, String> {
        Ok(OrderResponse {
            order_id: format!("stub-{}", req.client_id),
            status: "NEW".to_string(),
        })
    }

    fn cancel_order(&mut self, _order_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn cancel_all(&mut self) -> Result<(), String> {
        Ok(())
    }
}
