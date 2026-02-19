use arbitragefx::data::{file_sha256, EXPECTED_COLUMNS};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct HypothesisLedger {
    hypotheses: std::collections::HashMap<String, HypothesisEntry>,
}

#[derive(Debug, Deserialize)]
struct HypothesisEntry {
    id: String,
    statement: String,
    rationale: String,
    testable_prediction: String,
    success_criteria: serde_json::Value,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ManifestPayload {
    manifest: ManifestInner,
}

#[derive(Debug, Deserialize)]
struct ManifestInner {
    hash_sha256: String,
    columns: Vec<String>,
}

fn main() {
    let mut errors = Vec::new();

    // Required narrative artifacts
    for path in [
        "HYPOTHESIS_LEDGER.md",
        "EVIDENCE_LOG.md",
        "docs/ledger-workflow.md",
        "docs/backtest-loops.md",
    ] {
        if !Path::new(path).exists() {
            errors.push(format!("missing required file: {}", path));
        }
    }

    // Hypothesis ledger integrity (JSON)
    let ledger_path = Path::new("data/hypothesis_ledger.json");
    if ledger_path.exists() {
        match fs::read_to_string(ledger_path) {
            Ok(content) => match serde_json::from_str::<HypothesisLedger>(&content) {
                Ok(ledger) => {
                    if ledger.hypotheses.is_empty() {
                        errors.push("hypothesis_ledger.json has no entries".to_string());
                    }
                    for (key, entry) in ledger.hypotheses {
                        if entry.id != key {
                            errors.push(format!("hypothesis id mismatch: {} != {}", entry.id, key));
                        }
                        if entry.statement.trim().is_empty() {
                            errors.push(format!("hypothesis {} missing statement", entry.id));
                        }
                        if entry.rationale.trim().is_empty() {
                            errors.push(format!("hypothesis {} missing rationale", entry.id));
                        }
                        if entry.testable_prediction.trim().is_empty() {
                            errors.push(format!("hypothesis {} missing testable_prediction", entry.id));
                        }
                        if entry.status.trim().is_empty() {
                            errors.push(format!("hypothesis {} missing status", entry.id));
                        }
                        if entry.success_criteria.is_null() {
                            errors.push(format!("hypothesis {} missing success_criteria", entry.id));
                        }
                    }
                }
                Err(err) => errors.push(format!("invalid hypothesis_ledger.json: {}", err)),
            },
            Err(err) => errors.push(format!("failed to read hypothesis_ledger.json: {}", err)),
        }
    } else {
        errors.push("missing data/hypothesis_ledger.json".to_string());
    }

    // Dataset manifests (strict optional)
    let require_manifests = std::env::var("REQUIRE_MANIFESTS")
        .map(|v| v == "1")
        .unwrap_or(false);
    if require_manifests {
        let data_dir = Path::new("data");
        if let Ok(entries) = fs::read_dir(data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("csv") {
                    continue;
                }
                let manifest_path = manifest_path_for(&path);
                if !manifest_path.exists() {
                    errors.push(format!(
                        "missing manifest for {}",
                        path.display()
                    ));
                    continue;
                }
                match fs::read_to_string(&manifest_path) {
                    Ok(content) => match serde_json::from_str::<ManifestPayload>(&content) {
                        Ok(payload) => {
                            match file_sha256(&path) {
                                Ok(hash) => {
                                    if hash != payload.manifest.hash_sha256 {
                                        errors.push(format!(
                                            "hash mismatch for {}",
                                            path.display()
                                        ));
                                    }
                                }
                                Err(err) => errors.push(format!(
                                    "hash error for {}: {}",
                                    path.display(),
                                    err
                                )),
                            }
                            let expected: Vec<String> = EXPECTED_COLUMNS
                                .iter()
                                .map(|s| s.to_string())
                                .collect();
                            if payload.manifest.columns != expected {
                                errors.push(format!(
                                    "schema mismatch in manifest for {}",
                                    path.display()
                                ));
                            }
                        }
                        Err(err) => errors.push(format!(
                            "invalid manifest {}: {}",
                            manifest_path.display(),
                            err
                        )),
                    },
                    Err(err) => errors.push(format!(
                        "failed to read manifest {}: {}",
                        manifest_path.display(),
                        err
                    )),
                }
            }
        }
    }

    if errors.is_empty() {
        println!("coherence_check: ok");
        std::process::exit(0);
    }

    eprintln!("coherence_check: failed");
    for err in errors {
        eprintln!("- {}", err);
    }
    std::process::exit(2);
}

fn manifest_path_for(csv_path: &Path) -> PathBuf {
    let mut p = csv_path.to_path_buf();
    let fname = csv_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("dataset.csv");
    p.set_file_name(format!("{}.manifest.json", fname));
    p
}
