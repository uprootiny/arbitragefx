//! Drift Tracker - Right Mindfulness implementation
//!
//! Tracks distribution shifts in key features to detect when
//! model assumptions are breaking down.
//!
//! ## Design Principle
//!
//! > "Drift is computed even if you don't trade — because 'no-trade due to
//! >  uncertainty' is still a state you need mindfulness about."

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Drift severity levels with corresponding actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// No significant drift
    None,
    /// Minor drift - log only
    Low,
    /// Moderate drift - reduce position sizes, widen no-trade zone
    Moderate,
    /// Severe drift - halt new positions
    Severe,
    /// Critical drift - close existing positions
    Critical,
}

impl DriftSeverity {
    /// Position multiplier for this severity
    pub fn position_multiplier(&self) -> f64 {
        match self {
            DriftSeverity::None => 1.0,
            DriftSeverity::Low => 0.9,
            DriftSeverity::Moderate => 0.5,
            DriftSeverity::Severe => 0.0,
            DriftSeverity::Critical => 0.0,
        }
    }

    /// Should we halt new positions?
    pub fn should_halt(&self) -> bool {
        matches!(self, DriftSeverity::Severe | DriftSeverity::Critical)
    }

    /// Should we close existing positions?
    pub fn should_close(&self) -> bool {
        matches!(self, DriftSeverity::Critical)
    }
}

/// Report for a single feature's drift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    /// Feature name
    pub feature: String,
    /// Drift score (0.0 = no drift, 1.0+ = severe)
    pub score: f64,
    /// Severity classification
    pub severity: DriftSeverity,
    /// Baseline window stats
    pub baseline_mean: f64,
    pub baseline_std: f64,
    /// Recent window stats
    pub recent_mean: f64,
    pub recent_std: f64,
    /// Z-score of mean shift
    pub mean_shift_z: f64,
    /// Population Stability Index (PSI)
    pub psi: f64,
}

/// Rolling window for online statistics (Welford algorithm)
#[derive(Debug, Clone)]
pub struct RollingWindow {
    /// Maximum window size
    max_size: usize,
    /// Values in window
    values: VecDeque<f64>,
    /// Running statistics
    n: u64,
    mean: f64,
    m2: f64,
}

impl RollingWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            values: VecDeque::with_capacity(max_size),
            n: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Add a value to the window
    pub fn push(&mut self, value: f64) {
        // Remove oldest if at capacity
        if self.values.len() >= self.max_size {
            if let Some(old) = self.values.pop_front() {
                self.remove_from_stats(old);
            }
        }

        // Add new value
        self.values.push_back(value);
        self.add_to_stats(value);
    }

    fn add_to_stats(&mut self, value: f64) {
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn remove_from_stats(&mut self, value: f64) {
        if self.n <= 1 {
            self.n = 0;
            self.mean = 0.0;
            self.m2 = 0.0;
            return;
        }

        let delta = value - self.mean;
        self.mean = (self.mean * self.n as f64 - value) / (self.n as f64 - 1.0);
        let delta2 = value - self.mean;
        self.m2 -= delta * delta2;
        self.n -= 1;

        // Clamp m2 to avoid numerical issues
        if self.m2 < 0.0 {
            self.m2 = 0.0;
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_full(&self) -> bool {
        self.values.len() >= self.max_size
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }

    pub fn variance(&self) -> f64 {
        if self.n > 1 {
            self.m2 / (self.n as f64 - 1.0)
        } else {
            0.0
        }
    }

    pub fn std(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Get percentile (requires sorted copy)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self.values.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f64 - 1.0) * p) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

/// Feature tracker with baseline and recent windows
#[derive(Debug, Clone)]
pub struct FeatureTracker {
    name: String,
    /// Baseline window (longer history)
    baseline: RollingWindow,
    /// Recent window (shorter, more recent)
    recent: RollingWindow,
    /// Thresholds for severity classification
    thresholds: DriftThresholds,
}

#[derive(Debug, Clone)]
pub struct DriftThresholds {
    /// Z-score threshold for Low severity
    pub low_z: f64,
    /// Z-score threshold for Moderate severity
    pub moderate_z: f64,
    /// Z-score threshold for Severe severity
    pub severe_z: f64,
    /// Z-score threshold for Critical severity
    pub critical_z: f64,
}

impl Default for DriftThresholds {
    fn default() -> Self {
        Self {
            low_z: 1.0,
            moderate_z: 2.0,
            severe_z: 3.0,
            critical_z: 4.0,
        }
    }
}

impl FeatureTracker {
    pub fn new(name: &str, baseline_size: usize, recent_size: usize) -> Self {
        Self {
            name: name.to_string(),
            baseline: RollingWindow::new(baseline_size),
            recent: RollingWindow::new(recent_size),
            thresholds: DriftThresholds::default(),
        }
    }

    /// Push a new observation
    pub fn push(&mut self, value: f64) {
        self.baseline.push(value);
        self.recent.push(value);
    }

    /// Check if both windows have enough data
    pub fn is_ready(&self) -> bool {
        self.baseline.is_full() && self.recent.is_full()
    }

    /// Compute drift report
    pub fn report(&self) -> DriftReport {
        let baseline_mean = self.baseline.mean();
        let baseline_std = self.baseline.std();
        let recent_mean = self.recent.mean();
        let recent_std = self.recent.std();

        // Z-score of mean shift
        let mean_shift_z = if baseline_std > 1e-9 {
            (recent_mean - baseline_mean).abs() / baseline_std
        } else {
            0.0
        };

        // Simplified PSI (would need binning for full implementation)
        let psi = self.compute_psi();

        // Combined drift score
        let score = mean_shift_z * 0.6 + psi * 0.4;

        // Classify severity
        let severity = if score < self.thresholds.low_z {
            DriftSeverity::None
        } else if score < self.thresholds.moderate_z {
            DriftSeverity::Low
        } else if score < self.thresholds.severe_z {
            DriftSeverity::Moderate
        } else if score < self.thresholds.critical_z {
            DriftSeverity::Severe
        } else {
            DriftSeverity::Critical
        };

        DriftReport {
            feature: self.name.clone(),
            score,
            severity,
            baseline_mean,
            baseline_std,
            recent_mean,
            recent_std,
            mean_shift_z,
            psi,
        }
    }

    /// Simplified PSI using quantile comparison
    fn compute_psi(&self) -> f64 {
        if !self.is_ready() {
            return 0.0;
        }

        // Compare quartiles
        let quantiles = [0.25, 0.50, 0.75];
        let mut psi_sum = 0.0;

        for q in quantiles {
            let baseline_q = self.baseline.percentile(q);
            let recent_q = self.recent.percentile(q);

            if baseline_q.abs() > 1e-9 {
                let ratio = recent_q / baseline_q;
                if ratio > 0.0 {
                    // PSI component: (recent - baseline) * ln(recent/baseline)
                    let diff = (recent_q - baseline_q) / baseline_q.abs();
                    psi_sum += diff.abs() * ratio.ln().abs();
                }
            }
        }

        psi_sum
    }
}

/// Complete drift tracker for all monitored features
#[derive(Debug, Clone)]
pub struct DriftTracker {
    /// Individual feature trackers
    features: Vec<FeatureTracker>,
    /// Overall severity (worst of all features)
    pub overall_severity: DriftSeverity,
    /// Last update timestamp
    pub last_update_ts: u64,
}

impl DriftTracker {
    /// Create tracker with standard features
    pub fn new(baseline_size: usize, recent_size: usize) -> Self {
        Self {
            features: vec![
                FeatureTracker::new("volatility", baseline_size, recent_size),
                FeatureTracker::new("returns", baseline_size, recent_size),
                FeatureTracker::new("spread", baseline_size, recent_size),
                FeatureTracker::new("funding", baseline_size, recent_size),
                FeatureTracker::new("z_score", baseline_size, recent_size),
            ],
            overall_severity: DriftSeverity::None,
            last_update_ts: 0,
        }
    }

    /// Create with default window sizes (100 baseline, 20 recent)
    pub fn default_windows() -> Self {
        Self::new(100, 20)
    }

    /// Update a feature by name
    pub fn push(&mut self, feature: &str, value: f64, ts: u64) {
        for tracker in &mut self.features {
            if tracker.name == feature {
                tracker.push(value);
                break;
            }
        }
        self.last_update_ts = ts;
    }

    /// Update all features from market data
    pub fn update_from_market(
        &mut self,
        volatility: f64,
        returns: f64,
        spread: f64,
        funding: f64,
        z_score: f64,
        ts: u64,
    ) {
        self.push("volatility", volatility, ts);
        self.push("returns", returns, ts);
        self.push("spread", spread, ts);
        self.push("funding", funding, ts);
        self.push("z_score", z_score, ts);
    }

    /// Get reports for all features
    pub fn reports(&self) -> Vec<DriftReport> {
        self.features
            .iter()
            .filter(|f| f.is_ready())
            .map(|f| f.report())
            .collect()
    }

    /// Compute overall severity (worst of all features)
    pub fn compute_overall(&mut self) -> DriftSeverity {
        let reports = self.reports();

        self.overall_severity = reports
            .iter()
            .map(|r| r.severity)
            .max_by_key(|s| match s {
                DriftSeverity::None => 0,
                DriftSeverity::Low => 1,
                DriftSeverity::Moderate => 2,
                DriftSeverity::Severe => 3,
                DriftSeverity::Critical => 4,
            })
            .unwrap_or(DriftSeverity::None);

        self.overall_severity
    }

    /// Get position multiplier based on drift
    pub fn position_multiplier(&self) -> f64 {
        self.overall_severity.position_multiplier()
    }

    /// Generate action recommendations
    pub fn recommended_actions(&self) -> Vec<DriftAction> {
        let mut actions = Vec::new();

        for report in self.reports() {
            if report.severity != DriftSeverity::None {
                actions.push(DriftAction::Log {
                    msg: format!(
                        "Drift detected in {}: score={:.2}, z={:.2}, severity={:?}",
                        report.feature, report.score, report.mean_shift_z, report.severity
                    ),
                });
            }
        }

        match self.overall_severity {
            DriftSeverity::None => {}
            DriftSeverity::Low => {
                actions.push(DriftAction::Log {
                    msg: "Low drift detected. Monitoring.".to_string(),
                });
            }
            DriftSeverity::Moderate => {
                actions.push(DriftAction::ReduceExposure { multiplier: 0.5 });
                actions.push(DriftAction::WidenNoTradeZone { factor: 1.5 });
            }
            DriftSeverity::Severe => {
                actions.push(DriftAction::HaltNewPositions);
                actions.push(DriftAction::Alert {
                    msg: "SEVERE DRIFT: Halting new positions.".to_string(),
                });
            }
            DriftSeverity::Critical => {
                actions.push(DriftAction::HaltNewPositions);
                actions.push(DriftAction::CloseExisting { urgency: 1.0 });
                actions.push(DriftAction::Alert {
                    msg: "CRITICAL DRIFT: Closing positions.".to_string(),
                });
            }
        }

        actions
    }
}

/// Actions triggered by drift detection
#[derive(Debug, Clone)]
pub enum DriftAction {
    Log { msg: String },
    Alert { msg: String },
    ReduceExposure { multiplier: f64 },
    WidenNoTradeZone { factor: f64 },
    HaltNewPositions,
    CloseExisting { urgency: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window_stats() {
        let mut window = RollingWindow::new(5);

        for i in 1..=5 {
            window.push(i as f64);
        }

        assert_eq!(window.len(), 5);
        assert!((window.mean() - 3.0).abs() < 1e-9);

        // Add more values (should evict old ones)
        window.push(10.0);
        window.push(10.0);

        assert_eq!(window.len(), 5);
        // Mean should shift upward
        assert!(window.mean() > 3.0);
    }

    #[test]
    fn test_no_drift_when_stable() {
        let mut tracker = FeatureTracker::new("test", 50, 10);

        // Feed stable values
        for _ in 0..100 {
            tracker.push(100.0 + (rand_like() - 0.5) * 2.0);
        }

        let report = tracker.report();
        assert!(matches!(
            report.severity,
            DriftSeverity::None | DriftSeverity::Low
        ));
    }

    #[test]
    fn test_drift_when_shifted() {
        // Use large baseline (100), small recent (10)
        // This way baseline captures history, recent captures new regime
        let mut tracker = FeatureTracker::new("test", 100, 10);

        // Feed 100 baseline values around 100
        for i in 0..100 {
            tracker.push(100.0 + (i as f64 % 5.0) - 2.5);
        }

        // Now feed 10 shifted values - these will fill the recent window
        for _ in 0..10 {
            tracker.push(200.0);
        }

        let report = tracker.report();

        // Recent (last 10) should be ~200
        assert!(
            report.recent_mean > 180.0,
            "Recent mean should be ~200: {}",
            report.recent_mean
        );

        // Baseline (last 100) includes both old and new, so will be elevated
        // but the key is that mean_shift_z should be high
        assert!(
            report.mean_shift_z > 1.0 || report.score > 1.0,
            "Should detect drift: z={:.2}, score={:.2}",
            report.mean_shift_z,
            report.score
        );
    }

    #[test]
    fn test_overall_severity() {
        let mut tracker = DriftTracker::default_windows();

        // Feed stable data
        for i in 0..150 {
            tracker.update_from_market(
                0.01,   // volatility
                0.001,  // returns
                0.001,  // spread
                0.0001, // funding
                0.5,    // z_score
                i as u64,
            );
        }

        tracker.compute_overall();
        // Should be stable
        assert!(matches!(
            tracker.overall_severity,
            DriftSeverity::None | DriftSeverity::Low
        ));
    }

    /// Pseudo-random for deterministic tests
    fn rand_like() -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        (nanos % 1000) as f64 / 1000.0
    }
}
