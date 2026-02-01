#[derive(Debug, Clone)]
pub struct FaultProfile {
    pub timeout_rate: f64,
    pub dup_fill_rate: f64,
    pub drop_fill_rate: f64,
}

impl FaultProfile {
    pub fn disabled() -> Self {
        Self {
            timeout_rate: 0.0,
            dup_fill_rate: 0.0,
            drop_fill_rate: 0.0,
        }
    }
}

pub fn should_fault(seed: u64, rate: f64) -> bool {
    let v = (seed % 10_000) as f64 / 10_000.0;
    v < rate
}
