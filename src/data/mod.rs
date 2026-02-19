use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub const EXPECTED_COLUMNS: [&str; 11] = [
    "ts", "open", "high", "low", "close", "volume", "funding", "borrow", "liq", "depeg", "oi",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub start_ts: u64,
    pub end_ts: u64,
    pub missing_bars: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub path: String,
    pub hash_sha256: String,
    pub row_count: u64,
    pub bad_rows: u64,
    pub ts_min: Option<u64>,
    pub ts_max: Option<u64>,
    pub interval_secs: u64,
    pub columns: Vec<String>,
    pub gaps: Vec<Gap>,
    pub warnings: Vec<String>,
    pub ttl_secs: u64,
    pub stale: bool,
    pub generated_at_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaReport {
    pub columns: Vec<String>,
    pub expected: Vec<String>,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub rows: u64,
    pub bad_rows: u64,
    pub gaps: u64,
    pub stale: bool,
    pub warnings: Vec<String>,
}

pub fn analyze_csv(
    path: &Path,
    interval_secs: u64,
    ttl_secs: u64,
    now_ts: u64,
) -> Result<(DatasetManifest, DataQualityReport), String> {
    let mut warnings = Vec::new();
    let hash = file_sha256(path)?;

    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut row_count = 0u64;
    let mut bad_rows = 0u64;
    let mut ts_min: Option<u64> = None;
    let mut ts_max: Option<u64> = None;
    let mut prev_ts: Option<u64> = None;
    let mut gaps: Vec<Gap> = Vec::new();
    let mut header: Vec<String> = Vec::new();

    for line in reader.lines().flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.to_lowercase().starts_with("ts,") && header.is_empty() {
            header = trimmed
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            continue;
        }
        match parse_ts(trimmed) {
            Ok(ts) => {
                row_count += 1;
                ts_min = Some(ts_min.map(|v| v.min(ts)).unwrap_or(ts));
                ts_max = Some(ts_max.map(|v| v.max(ts)).unwrap_or(ts));
                if let Some(prev) = prev_ts {
                    if ts > prev && ts - prev > interval_secs {
                        let missing = (ts - prev) / interval_secs - 1;
                        gaps.push(Gap {
                            start_ts: prev,
                            end_ts: ts,
                            missing_bars: missing,
                        });
                    } else if ts <= prev {
                        warnings.push(format!("non_monotonic_ts: prev={} current={}", prev, ts));
                    }
                }
                prev_ts = Some(ts);
            }
            Err(err) => {
                bad_rows += 1;
                warnings.push(format!("bad_row: {}", err));
            }
        }
    }

    if header.is_empty() {
        warnings.push("missing_header".to_string());
    }

    let stale = ts_max
        .map(|ts| now_ts.saturating_sub(ts) > ttl_secs)
        .unwrap_or(true);

    let manifest = DatasetManifest {
        path: path.display().to_string(),
        hash_sha256: hash,
        row_count,
        bad_rows,
        ts_min,
        ts_max,
        interval_secs,
        columns: header.clone(),
        gaps: gaps.clone(),
        warnings: warnings.clone(),
        ttl_secs,
        stale,
        generated_at_epoch: now_ts,
    };

    let report = DataQualityReport {
        rows: row_count,
        bad_rows,
        gaps: gaps.len() as u64,
        stale,
        warnings,
    };

    Ok((manifest, report))
}

pub fn validate_schema(path: &Path) -> Result<SchemaReport, String> {
    let header = read_header(path)?;
    let expected = EXPECTED_COLUMNS.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let ok = header == expected;
    let message = if ok {
        "schema ok".to_string()
    } else {
        format!("schema mismatch: got {:?} expected {:?}", header, expected)
    };
    Ok(SchemaReport {
        columns: header,
        expected,
        ok,
        message,
    })
}

pub fn read_header(path: &Path) -> Result<Vec<String>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    for line in reader.lines().flatten() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.to_lowercase().starts_with("ts,") {
            return Ok(trimmed.split(',').map(|s| s.trim().to_string()).collect());
        }
        break;
    }
    Ok(Vec::new())
}

fn parse_ts(line: &str) -> Result<u64, String> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 10 {
        return Err(format!("expected 10+ columns, got {}", parts.len()));
    }
    parts[0]
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("bad ts: {}", e))
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn default_manifest_path(dataset_path: &Path) -> PathBuf {
    let mut p = dataset_path.to_path_buf();
    let fname = dataset_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("dataset.csv");
    p.set_file_name(format!("{}.manifest.json", fname));
    p
}
