use super::strategy::Action;

#[derive(Debug, Clone, Copy)]
pub struct Fill {
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
}

pub trait Execution {
    fn place(&mut self, price: f64, action: Action) -> Fill;
}

#[derive(Debug, Clone, Copy)]
pub struct PaperExec {
    pub fee_rate: f64,
    pub slip_rate: f64,
}

impl Execution for PaperExec {
    fn place(&mut self, price: f64, action: Action) -> Fill {
        match action {
            Action::Buy { qty } => Fill {
                price: price * (1.0 + self.slip_rate),
                qty,
                fee: qty * price * self.fee_rate,
            },
            Action::Sell { qty } => Fill {
                price: price * (1.0 - self.slip_rate),
                qty: -qty,
                fee: qty * price * self.fee_rate,
            },
            Action::Close => Fill { price, qty: 0.0, fee: 0.0 },
            Action::Hold => Fill { price, qty: 0.0, fee: 0.0 },
        }
    }
}
