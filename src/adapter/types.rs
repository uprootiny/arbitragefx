#[derive(Debug, Clone, Copy)]
pub enum VenueKind {
    Cex,
    Dex,
}

#[derive(Debug, Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub qty: f64,
    pub client_id: String,
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct FillEvent {
    pub order_id: String,
    pub fill_id: String,
    pub qty: f64,
    pub price: f64,
    pub fee: f64,
    pub ts: u64,
}
