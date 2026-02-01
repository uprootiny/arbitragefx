use std::fs::File;
use std::io::{BufRead, BufReader};

use arbitragefx::backtest::{parse_csv_line, run_backtest};
use arbitragefx::state::Config;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "data.csv".to_string());
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("failed to open {}: {}", path, err);
            return;
        }
    };
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().flatten() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if line.to_lowercase().starts_with("ts,") {
            continue;
        }
        match parse_csv_line(&line) {
            Ok(r) => rows.push(r),
            Err(err) => eprintln!("bad row: {}", err),
        }
    }
    if rows.is_empty() {
        eprintln!("no rows parsed");
        return;
    }
    let cfg = Config::from_env();
    match run_backtest(cfg, &rows) {
        Ok((pnl, dd)) => println!("pnl_total={:.4} max_drawdown={:.4}", pnl, dd),
        Err(err) => eprintln!("backtest failed: {}", err),
    }
}
