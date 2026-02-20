//! Walk-forward validation binary.
//!
//! Usage: cargo run --bin walk_forward -- data/btc_real_1h.csv [windows] [train_fraction]

use arbitragefx::backtest::{parse_csv_line, CsvRow};
use arbitragefx::state::Config;
use arbitragefx::walk_forward;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let csv_path = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("data/btc_real_1h.csv");
    let num_windows: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let train_fraction: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.7);

    let content = std::fs::read_to_string(csv_path).expect("failed to read CSV");
    let rows: Vec<CsvRow> = content
        .lines()
        .filter(|l| !l.starts_with("ts") && !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| parse_csv_line(l).ok())
        .collect();

    println!("Walk-Forward Validation");
    println!("=======================");
    println!("Data: {} ({} candles)", csv_path, rows.len());
    println!(
        "Windows: {}, Train fraction: {:.0}%",
        num_windows,
        train_fraction * 100.0
    );
    println!();

    let cfg = Config::from_env();
    let result = walk_forward::walk_forward(cfg, &rows, num_windows, train_fraction)
        .expect("walk-forward failed");

    println!("Windows produced: {}", result.num_windows);
    println!(
        "Comparisons: {} ({} strategies x {} windows)",
        result.num_comparisons, result.num_strategies, result.num_windows
    );
    println!(
        "Correction: {} (alpha = {})",
        result.correction_method, result.alpha
    );
    println!();

    println!(
        "{:<14} {:>10} {:>10} {:>8} {:>6}/{:<6} {:>8} {:>10}",
        "Strategy", "Train PnL", "Test PnL", "Overfit", "Win", "Total", "p-value", "Survives?"
    );
    println!("{}", "-".repeat(80));

    for s in &result.summaries {
        println!(
            "{:<14} {:>10.4} {:>10.4} {:>8.2} {:>6}/{:<6} {:>8.4} {:>10}",
            s.id,
            s.train_mean_pnl,
            s.test_mean_pnl,
            s.overfit_ratio,
            s.test_positive_windows,
            s.total_windows,
            s.p_value,
            if s.survives_correction { "YES" } else { "no" },
        );
    }

    let survivors: Vec<_> = result
        .summaries
        .iter()
        .filter(|s| s.survives_correction)
        .collect();

    println!();
    if survivors.is_empty() {
        println!(
            "No strategies survive Bonferroni correction at alpha={}.",
            result.alpha
        );
        println!("This is the honest answer: we cannot reject the null hypothesis");
        println!("that these strategies perform no better than random.");
    } else {
        println!("{} strategies survive correction:", survivors.len());
        for s in &survivors {
            println!(
                "  {} (test mean PnL: {:.4}, overfit ratio: {:.2})",
                s.id, s.test_mean_pnl, s.overfit_ratio
            );
        }
    }

    // Write JSON report
    let json = result.to_json();
    let report_dir = "out/walk_forward";
    let _ = std::fs::create_dir_all(report_dir);
    let report_path = format!("{}/report.json", report_dir);
    if std::fs::write(&report_path, &json).is_ok() {
        println!("\nJSON report: {}", report_path);
    }
}
