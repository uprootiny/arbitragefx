//! Experiment Registry - Right Speech implementation
//!
//! All trials logged, no cherry-picking. This prevents self-delusion
//! by selection bias.
//!
//! ## Output Structure
//! ```text
//! out/experiments/<run_id>/
//!   manifest.json    - run metadata
//!   trials.jsonl     - one line per trial (append-only)
//!   summary.json     - computed after all trials
//! ```

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Experiment run metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRun {
    /// Unique run identifier
    pub id: String,
    /// Git SHA at time of run (if available)
    pub git_sha: Option<String>,
    /// Start timestamp
    pub start_ts: u64,
    /// Random seed for reproducibility
    pub seed: u64,
    /// Dataset identifier
    pub dataset_id: String,
    /// Hash of configuration
    pub config_hash: u64,
    /// Description
    pub description: String,
    /// Multiple testing correction method
    pub correction_method: CorrectionMethod,
    /// Total planned trials (for Bonferroni)
    pub planned_trials: u32,
}

/// Multiple testing correction methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectionMethod {
    /// No correction - results labeled UNTRUSTWORTHY
    None,
    /// Bonferroni: alpha_adj = alpha / N
    Bonferroni,
    /// Holm-Bonferroni (step-down)
    Holm,
    /// False Discovery Rate (Benjamini-Hochberg)
    FDR,
    /// Two-stage: train selection + holdout validation
    TwoStage,
}

impl CorrectionMethod {
    /// Adjust alpha for multiple comparisons
    pub fn adjust_alpha(&self, base_alpha: f64, n_trials: u32) -> f64 {
        match self {
            CorrectionMethod::None => base_alpha,
            CorrectionMethod::Bonferroni => base_alpha / n_trials as f64,
            CorrectionMethod::Holm => base_alpha / n_trials as f64, // Simplified; real Holm is step-down
            CorrectionMethod::FDR => base_alpha, // FDR is applied post-hoc
            CorrectionMethod::TwoStage => base_alpha, // Validation handles it
        }
    }

    /// Is this method trustworthy for N trials?
    pub fn is_trustworthy(&self, n_trials: u32) -> bool {
        match self {
            CorrectionMethod::None => n_trials <= 1,
            _ => true,
        }
    }
}

/// Single trial result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    /// Run this trial belongs to
    pub run_id: String,
    /// Trial index within run
    pub trial_id: u32,
    /// Parameters tested
    pub params: HashMap<String, f64>,
    /// Metrics collected
    pub metrics: TrialMetrics,
    /// Trial status
    pub status: TrialStatus,
    /// Notes or warnings
    pub notes: Vec<String>,
    /// Timestamp
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialMetrics {
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub total_trades: u32,
    pub win_rate: f64,
    pub profit_factor: f64,
    /// Reality score from backtest_ethics
    pub reality_score: f64,
    /// Extra metrics
    pub extra: HashMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrialStatus {
    Completed,
    Failed,
    Skipped,
    Timeout,
}

/// Run summary (computed after all trials)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub total_trials: u32,
    pub completed_trials: u32,
    pub failed_trials: u32,
    /// Is this run trustworthy?
    pub trustworthy: bool,
    /// Why not trustworthy (if applicable)
    pub trust_issues: Vec<String>,
    /// Best trial by reality score (not return!)
    pub best_by_reality: Option<u32>,
    /// Metric distributions
    pub metric_distributions: HashMap<String, Distribution>,
    /// Corrected significance threshold
    pub corrected_alpha: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Distribution {
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub p25: f64,
    pub p75: f64,
}

/// Experiment registry - manages run lifecycle
pub struct ExperimentRegistry {
    base_path: PathBuf,
    current_run: Option<ExperimentRun>,
    trials_writer: Option<BufWriter<File>>,
    trial_count: u32,
}

impl ExperimentRegistry {
    pub fn new<P: AsRef<Path>>(base_path: P) -> std::io::Result<Self> {
        let base = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base)?;
        Ok(Self {
            base_path: base,
            current_run: None,
            trials_writer: None,
            trial_count: 0,
        })
    }

    /// Start a new experiment run
    pub fn start_run(&mut self, run: ExperimentRun) -> std::io::Result<()> {
        let run_dir = self.base_path.join(&run.id);
        fs::create_dir_all(&run_dir)?;

        // Write manifest
        let manifest_path = run_dir.join("manifest.json");
        let manifest_file = File::create(manifest_path)?;
        serde_json::to_writer_pretty(manifest_file, &run)?;

        // Open trials.jsonl for append
        let trials_path = run_dir.join("trials.jsonl");
        let trials_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(trials_path)?;
        self.trials_writer = Some(BufWriter::new(trials_file));

        self.current_run = Some(run);
        self.trial_count = 0;

        Ok(())
    }

    /// Record a trial result (append-only)
    pub fn record_trial(&mut self, mut trial: TrialResult) -> std::io::Result<()> {
        if let Some(ref run) = self.current_run {
            trial.run_id = run.id.clone();
            trial.trial_id = self.trial_count;
            self.trial_count += 1;

            if let Some(ref mut writer) = self.trials_writer {
                let line = serde_json::to_string(&trial)?;
                writeln!(writer, "{}", line)?;
                writer.flush()?;
            }
        }
        Ok(())
    }

    /// Finish run and compute summary
    pub fn finish_run(&mut self, base_alpha: f64) -> std::io::Result<Option<RunSummary>> {
        // Flush writer
        if let Some(ref mut writer) = self.trials_writer {
            writer.flush()?;
        }
        self.trials_writer = None;

        let run = match self.current_run.take() {
            Some(r) => r,
            None => return Ok(None),
        };

        // Read all trials back
        let run_dir = self.base_path.join(&run.id);
        let trials_path = run_dir.join("trials.jsonl");
        let trials_file = File::open(&trials_path)?;
        let reader = BufReader::new(trials_file);

        let mut trials: Vec<TrialResult> = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(trial) = serde_json::from_str(&line) {
                trials.push(trial);
            }
        }

        // Compute summary
        let summary = self.compute_summary(&run, &trials, base_alpha);

        // Write summary
        let summary_path = run_dir.join("summary.json");
        let summary_file = File::create(summary_path)?;
        serde_json::to_writer_pretty(summary_file, &summary)?;

        Ok(Some(summary))
    }

    fn compute_summary(&self, run: &ExperimentRun, trials: &[TrialResult], base_alpha: f64) -> RunSummary {
        let completed: Vec<_> = trials.iter()
            .filter(|t| t.status == TrialStatus::Completed)
            .collect();

        let n = trials.len() as u32;
        let corrected_alpha = run.correction_method.adjust_alpha(base_alpha, n.max(1));

        // Check trustworthiness
        let mut trust_issues = Vec::new();
        let trustworthy = if !run.correction_method.is_trustworthy(n) {
            trust_issues.push(format!(
                "UNTRUSTWORTHY: {} trials with correction={:?}",
                n, run.correction_method
            ));
            false
        } else {
            true
        };

        // Find best by reality score
        let best_by_reality = completed.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.metrics.reality_score.partial_cmp(&b.metrics.reality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i as u32);

        // Compute metric distributions
        let mut metric_distributions = HashMap::new();

        if !completed.is_empty() {
            let returns: Vec<f64> = completed.iter().map(|t| t.metrics.total_return_pct).collect();
            metric_distributions.insert("total_return_pct".to_string(), compute_distribution(&returns));

            let drawdowns: Vec<f64> = completed.iter().map(|t| t.metrics.max_drawdown_pct).collect();
            metric_distributions.insert("max_drawdown_pct".to_string(), compute_distribution(&drawdowns));

            let reality: Vec<f64> = completed.iter().map(|t| t.metrics.reality_score).collect();
            metric_distributions.insert("reality_score".to_string(), compute_distribution(&reality));
        }

        RunSummary {
            run_id: run.id.clone(),
            total_trials: n,
            completed_trials: completed.len() as u32,
            failed_trials: trials.iter().filter(|t| t.status == TrialStatus::Failed).count() as u32,
            trustworthy,
            trust_issues,
            best_by_reality,
            metric_distributions,
            corrected_alpha,
        }
    }
}

fn compute_distribution(values: &[f64]) -> Distribution {
    if values.is_empty() {
        return Distribution {
            mean: 0.0, std: 0.0, min: 0.0, max: 0.0,
            median: 0.0, p25: 0.0, p75: 0.0,
        };
    }

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std = variance.sqrt();

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let percentile = |p: f64| -> f64 {
        let idx = ((sorted.len() as f64 - 1.0) * p) as usize;
        sorted[idx.min(sorted.len() - 1)]
    };

    Distribution {
        mean,
        std,
        min: sorted[0],
        max: sorted[sorted.len() - 1],
        median: percentile(0.5),
        p25: percentile(0.25),
        p75: percentile(0.75),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ts() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    #[test]
    fn test_bonferroni_correction() {
        let method = CorrectionMethod::Bonferroni;
        assert!((method.adjust_alpha(0.05, 100) - 0.0005).abs() < 1e-9);
    }

    #[test]
    fn test_none_is_untrustworthy() {
        let method = CorrectionMethod::None;
        assert!(method.is_trustworthy(1));  // Single trial is OK
        assert!(!method.is_trustworthy(10)); // Multiple without correction = bad
    }

    #[test]
    fn test_registry_lifecycle() {
        let dir = std::env::temp_dir().join(format!("exp_test_{}", now_ts()));
        let mut registry = ExperimentRegistry::new(&dir).unwrap();

        let run = ExperimentRun {
            id: format!("test_run_{}", now_ts()),
            git_sha: Some("abc123".to_string()),
            start_ts: now_ts(),
            seed: 42,
            dataset_id: "btc_sim".to_string(),
            config_hash: 12345,
            description: "Test run".to_string(),
            correction_method: CorrectionMethod::Bonferroni,
            planned_trials: 10,
        };

        registry.start_run(run.clone()).unwrap();

        // Record some trials
        for i in 0..3 {
            let trial = TrialResult {
                run_id: String::new(), // Will be filled
                trial_id: 0,
                params: [("threshold".to_string(), 0.1 + i as f64 * 0.1)].into(),
                metrics: TrialMetrics {
                    total_return_pct: 0.05 + i as f64 * 0.02,
                    sharpe_ratio: 1.0,
                    max_drawdown_pct: 0.1,
                    total_trades: 10,
                    win_rate: 0.6,
                    profit_factor: 1.5,
                    reality_score: 0.7 + i as f64 * 0.05,
                    extra: HashMap::new(),
                },
                status: TrialStatus::Completed,
                notes: vec![],
                timestamp: now_ts(),
            };
            registry.record_trial(trial).unwrap();
        }

        let summary = registry.finish_run(0.05).unwrap().unwrap();

        assert_eq!(summary.total_trials, 3);
        assert_eq!(summary.completed_trials, 3);
        assert!(summary.trustworthy); // Bonferroni applied
        assert!(summary.best_by_reality.is_some());

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_untrustworthy_labeling() {
        let dir = std::env::temp_dir().join(format!("exp_untrust_{}", now_ts()));
        let mut registry = ExperimentRegistry::new(&dir).unwrap();

        let run = ExperimentRun {
            id: format!("untrust_run_{}", now_ts()),
            git_sha: None,
            start_ts: now_ts(),
            seed: 42,
            dataset_id: "test".to_string(),
            config_hash: 0,
            description: "Untrustworthy test".to_string(),
            correction_method: CorrectionMethod::None, // No correction!
            planned_trials: 10,
        };

        registry.start_run(run).unwrap();

        // Record multiple trials with no correction
        for i in 0..5 {
            let trial = TrialResult {
                run_id: String::new(),
                trial_id: 0,
                params: [("x".to_string(), i as f64)].into(),
                metrics: TrialMetrics {
                    total_return_pct: 0.1,
                    sharpe_ratio: 1.0,
                    max_drawdown_pct: 0.1,
                    total_trades: 10,
                    win_rate: 0.6,
                    profit_factor: 1.5,
                    reality_score: 0.7,
                    extra: HashMap::new(),
                },
                status: TrialStatus::Completed,
                notes: vec![],
                timestamp: now_ts(),
            };
            registry.record_trial(trial).unwrap();
        }

        let summary = registry.finish_run(0.05).unwrap().unwrap();

        // Must be labeled UNTRUSTWORTHY
        assert!(!summary.trustworthy);
        assert!(!summary.trust_issues.is_empty());
        assert!(summary.trust_issues[0].contains("UNTRUSTWORTHY"));

        let _ = fs::remove_dir_all(&dir);
    }
}
