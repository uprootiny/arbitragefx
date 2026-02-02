#[derive(Debug, Clone, Copy)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub state: CircuitState,
    pub failures: u32,
    pub threshold: u32,
}

impl CircuitBreaker {
    pub fn new(threshold: u32) -> Self {
        Self { state: CircuitState::Closed, failures: 0, threshold }
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
        self.state = CircuitState::Closed;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        if self.failures >= self.threshold {
            self.state = CircuitState::Open;
        }
    }

    pub fn allow(&self) -> bool {
        matches!(self.state, CircuitState::Closed | CircuitState::HalfOpen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_opens_on_threshold() {
        let mut cb = CircuitBreaker::new(3);
        assert!(cb.allow());
        cb.record_failure();
        cb.record_failure();
        assert!(cb.allow());
        cb.record_failure();
        assert!(!cb.allow());
        assert!(matches!(cb.state, CircuitState::Open));
    }

    #[test]
    fn test_circuit_resets_on_success() {
        let mut cb = CircuitBreaker::new(2);
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.allow());
        cb.record_success();
        assert!(cb.allow());
        assert!(matches!(cb.state, CircuitState::Closed));
    }
}
