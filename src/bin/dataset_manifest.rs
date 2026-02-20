use arbitragefx::data::{analyze_csv, default_manifest_path, validate_schema, EXPECTED_COLUMNS};
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "data/sample.csv".to_string());
    let interval_secs = env::var("DATA_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let ttl_secs = env::var("DATA_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600);

    let now_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let schema = match validate_schema(PathBuf::from(&path).as_path()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("schema check failed: {}", err);
            std::process::exit(1);
        }
    };

    if !schema.ok {
        eprintln!("schema mismatch: {:?}", schema.message);
        eprintln!("expected columns: {:?}", EXPECTED_COLUMNS);
        std::process::exit(2);
    }

    let (manifest, report) = match analyze_csv(
        PathBuf::from(&path).as_path(),
        interval_secs,
        ttl_secs,
        now_ts,
    ) {
        Ok(m) => m,
        Err(err) => {
            eprintln!("analysis failed: {}", err);
            std::process::exit(3);
        }
    };

    let out_path = default_manifest_path(PathBuf::from(&path).as_path());
    let payload = json!({
        "manifest": manifest,
        "report": report
    });
    if let Err(err) = fs::write(&out_path, serde_json::to_string_pretty(&payload).unwrap()) {
        eprintln!("failed to write {}: {}", out_path.display(), err);
        std::process::exit(4);
    }
    println!("wrote manifest {}", out_path.display());
}
