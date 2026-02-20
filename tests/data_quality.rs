use arbitragefx::data::{analyze_csv, validate_schema, EXPECTED_COLUMNS};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_csv(path: &Path, header: &[&str], rows: &[&str]) {
    let mut out = String::new();
    out.push_str(&header.join(","));
    out.push('\n');
    for row in rows {
        out.push_str(row);
        out.push('\n');
    }
    fs::write(path, out).unwrap();
}

#[test]
fn schema_accepts_good_header() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("good.csv");
    write_csv(
        &path,
        &EXPECTED_COLUMNS,
        &["1000,1,2,0.5,1.5,10,0.0,0.0,0.0,0.0,0.0"],
    );
    let report = validate_schema(&path).unwrap();
    assert!(report.ok);
}

#[test]
fn schema_rejects_bad_header() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.csv");
    write_csv(&path, &["ts", "open", "high", "low"], &["1000,1,2,0.5"]);
    let report = validate_schema(&path).unwrap();
    assert!(!report.ok);
}

#[test]
fn detects_gaps_and_staleness() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("gaps.csv");
    write_csv(
        &path,
        &EXPECTED_COLUMNS,
        &[
            "1000,1,2,0.5,1.5,10,0.0,0.0,0.0,0.0,0.0",
            "1120,1,2,0.5,1.5,10,0.0,0.0,0.0,0.0,0.0",
        ],
    );
    let (manifest, report) = analyze_csv(&path, 60, 30, 2000).unwrap();
    assert_eq!(manifest.gaps.len(), 1);
    assert!(report.stale);
}
