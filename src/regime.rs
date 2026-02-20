//! Regime classification from backtest data.
//!
//! Bridges the narrative_detector module with CsvRow candle data
//! to classify datasets into market regime categories.

use crate::backtest::CsvRow;
use crate::narrative_detector::{NarrativeIndicators, NarrativeRegime};
use serde::Serialize;

/// Summary of regime classification for a dataset.
#[derive(Debug, Clone, Serialize)]
pub struct RegimeSummary {
    /// Dominant regime across the dataset
    pub dominant_regime: String,
    /// Mean narrative score (0.0 = grounded, 1.0 = reflexive)
    pub mean_narrative_score: f64,
    /// Fraction of candles in each regime
    pub grounded_frac: f64,
    pub uncertain_frac: f64,
    pub narrative_frac: f64,
    pub reflexive_frac: f64,
    /// Price trend classification
    pub price_trend: String,
    /// Price change over dataset (%)
    pub price_change_pct: f64,
    /// Mean volatility ratio
    pub mean_volatility_ratio: f64,
}

/// Classify a dataset into its dominant market regime.
///
/// Processes candle data in rolling windows to compute narrative indicators
/// at each step, then aggregates into a single regime classification.
pub fn classify_dataset(rows: &[CsvRow]) -> RegimeSummary {
    if rows.len() < 20 {
        return RegimeSummary {
            dominant_regime: "insufficient_data".into(),
            mean_narrative_score: 0.0,
            grounded_frac: 1.0,
            uncertain_frac: 0.0,
            narrative_frac: 0.0,
            reflexive_frac: 0.0,
            price_trend: "unknown".into(),
            price_change_pct: 0.0,
            mean_volatility_ratio: 1.0,
        };
    }

    let mut scores = Vec::new();
    let mut regimes = Vec::new();
    let mut vol_ratios = Vec::new();

    // Rolling statistics for z-scores
    let mut funding_sum = 0.0;
    let mut funding_sq_sum = 0.0;
    let mut vol_sum = 0.0;
    let mut vol_sq_sum = 0.0;
    let warmup = 20;

    for (i, row) in rows.iter().enumerate() {
        // Compute rolling volatility from high-low range
        let candle_vol = if row.c > 0.0 { (row.h - row.l) / row.c } else { 0.0 };

        funding_sum += row.funding;
        funding_sq_sum += row.funding * row.funding;
        vol_sum += candle_vol;
        vol_sq_sum += candle_vol * candle_vol;

        if i < warmup {
            continue;
        }

        let n = (i + 1) as f64;
        let funding_mean = funding_sum / n;
        let funding_var = (funding_sq_sum / n - funding_mean * funding_mean).max(0.0);
        let funding_std = funding_var.sqrt().max(1e-9);
        let funding_zscore = (row.funding - funding_mean) / funding_std;

        let vol_mean = vol_sum / n;
        let vol_var = (vol_sq_sum / n - vol_mean * vol_mean).max(0.0);
        let vol_std = vol_var.sqrt().max(1e-9);
        let volatility_ratio = if vol_mean > 0.0 { candle_vol / vol_mean } else { 1.0 };

        // Compute OI change rate
        let oi_change_rate = if i > 0 && rows[i - 1].oi > 0.0 {
            (row.oi - rows[i - 1].oi) / rows[i - 1].oi
        } else {
            0.0
        };

        // Volume ratio (current vs rolling mean)
        let volume_ratio = if i >= warmup {
            let recent_vol: f64 = rows[i.saturating_sub(warmup)..i].iter().map(|r| r.v).sum::<f64>()
                / warmup as f64;
            if recent_vol > 0.0 { row.v / recent_vol } else { 1.0 }
        } else {
            1.0
        };

        // Price-volume divergence: large price move with low volume
        let price_change = if i > 0 && rows[i - 1].c > 0.0 {
            (row.c - rows[i - 1].c) / rows[i - 1].c
        } else {
            0.0
        };
        let pv_divergence = if volume_ratio < 0.5 { price_change.abs() } else { 0.0 };

        let indicators = NarrativeIndicators {
            funding_rate: row.funding,
            funding_avg: funding_mean,
            funding_zscore,
            liquidation_score: row.liq,
            liquidation_imbalance: 0.0,
            price_change_pct: price_change,
            volume_ratio,
            pv_divergence,
            volatility_ratio,
            vol_clustering: vol_std / vol_mean.max(1e-9),
            oi_change_rate,
            retail_flow_proxy: 0.0,
        };

        scores.push(indicators.narrative_score());
        regimes.push(indicators.regime());
        vol_ratios.push(volatility_ratio);
    }

    if scores.is_empty() {
        return RegimeSummary {
            dominant_regime: "insufficient_data".into(),
            mean_narrative_score: 0.0,
            grounded_frac: 1.0,
            uncertain_frac: 0.0,
            narrative_frac: 0.0,
            reflexive_frac: 0.0,
            price_trend: "unknown".into(),
            price_change_pct: 0.0,
            mean_volatility_ratio: 1.0,
        };
    }

    let n = scores.len() as f64;
    let mean_score = scores.iter().sum::<f64>() / n;
    let mean_vol = vol_ratios.iter().sum::<f64>() / n;

    let grounded = regimes.iter().filter(|r| **r == NarrativeRegime::Grounded).count() as f64 / n;
    let uncertain = regimes.iter().filter(|r| **r == NarrativeRegime::Uncertain).count() as f64 / n;
    let narrative = regimes.iter().filter(|r| **r == NarrativeRegime::NarrativeDriven).count() as f64 / n;
    let reflexive = regimes.iter().filter(|r| **r == NarrativeRegime::Reflexive).count() as f64 / n;

    // Price trend
    let first_price = rows.first().map(|r| r.c).unwrap_or(0.0);
    let last_price = rows.last().map(|r| r.c).unwrap_or(0.0);
    let price_change_pct = if first_price > 0.0 {
        (last_price - first_price) / first_price * 100.0
    } else {
        0.0
    };
    let price_trend = if price_change_pct > 10.0 {
        "strong_bull"
    } else if price_change_pct > 2.0 {
        "mild_bull"
    } else if price_change_pct < -10.0 {
        "strong_bear"
    } else if price_change_pct < -2.0 {
        "mild_bear"
    } else {
        "ranging"
    };

    // Dominant regime: most common
    let dominant = if grounded >= uncertain && grounded >= narrative && grounded >= reflexive {
        "grounded"
    } else if uncertain >= narrative && uncertain >= reflexive {
        "uncertain"
    } else if narrative >= reflexive {
        "narrative_driven"
    } else {
        "reflexive"
    };

    RegimeSummary {
        dominant_regime: dominant.into(),
        mean_narrative_score: mean_score,
        grounded_frac: grounded,
        uncertain_frac: uncertain,
        narrative_frac: narrative,
        reflexive_frac: reflexive,
        price_trend: price_trend.into(),
        price_change_pct,
        mean_volatility_ratio: mean_vol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rows(prices: &[f64], funding: f64) -> Vec<CsvRow> {
        prices.iter().enumerate().map(|(i, &c)| CsvRow {
            ts: 1000 + i as u64 * 3600,
            o: c * 0.99,
            h: c * 1.01,
            l: c * 0.98,
            c,
            v: 1000.0,
            funding,
            borrow: 0.0,
            liq: 0.0,
            depeg: 0.0,
            oi: 50000.0,
        }).collect()
    }

    #[test]
    fn test_classify_bull_trend() {
        // Steady uptrend: 100 → 150 over 100 candles
        let prices: Vec<f64> = (0..100).map(|i| 100.0 + i as f64 * 0.5).collect();
        let rows = make_rows(&prices, 0.0001);
        let summary = classify_dataset(&rows);
        assert_eq!(summary.price_trend, "strong_bull");
        assert!(summary.price_change_pct > 10.0);
    }

    #[test]
    fn test_classify_bear_trend() {
        // Steady downtrend: 100 → 60
        let prices: Vec<f64> = (0..100).map(|i| 100.0 - i as f64 * 0.4).collect();
        let rows = make_rows(&prices, -0.0001);
        let summary = classify_dataset(&rows);
        assert_eq!(summary.price_trend, "strong_bear");
        assert!(summary.price_change_pct < -10.0);
    }

    #[test]
    fn test_classify_ranging() {
        // Oscillating around 100
        let prices: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 2.0).collect();
        let rows = make_rows(&prices, 0.0);
        let summary = classify_dataset(&rows);
        assert_eq!(summary.price_trend, "ranging");
    }

    #[test]
    fn test_classify_grounded_regime() {
        // Calm market with low funding, no liquidations
        let prices: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.3).sin()).collect();
        let rows = make_rows(&prices, 0.0001);
        let summary = classify_dataset(&rows);
        assert_eq!(summary.dominant_regime, "grounded");
        assert!(summary.grounded_frac > 0.5);
    }

    #[test]
    fn test_classify_insufficient_data() {
        let rows = make_rows(&[100.0, 101.0, 99.0], 0.0);
        let summary = classify_dataset(&rows);
        assert_eq!(summary.dominant_regime, "insufficient_data");
    }

    #[test]
    fn test_classify_real_data() {
        let csv = "data/btc_real_1h.csv";
        if !std::path::Path::new(csv).exists() {
            return;
        }
        let content = std::fs::read_to_string(csv).unwrap();
        let rows: Vec<CsvRow> = content.lines()
            .filter(|l| !l.starts_with("ts") && !l.starts_with('#') && !l.is_empty())
            .filter_map(|l| crate::backtest::parse_csv_line(l).ok())
            .collect();
        let summary = classify_dataset(&rows);
        // Should classify as something meaningful
        assert_ne!(summary.dominant_regime, "insufficient_data");
        assert!(summary.mean_narrative_score >= 0.0 && summary.mean_narrative_score <= 1.0);
        assert!((summary.grounded_frac + summary.uncertain_frac + summary.narrative_frac + summary.reflexive_frac - 1.0).abs() < 0.01);
    }
}
