use chrono::Utc;
use serde_json::{json, Map, Value};
use std::time::Instant;

pub fn ts_now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn json_log(module: &str, mut fields: Map<String, Value>) {
    fields.insert("ts".to_string(), Value::String(ts_now()));
    fields.insert("module".to_string(), Value::String(module.to_string()));
    let out = Value::Object(fields);
    println!("{}", out.to_string());
}

pub fn params_hash(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}

pub fn obj(pairs: &[(&str, Value)]) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), v.clone());
    }
    map
}

pub fn v_str(s: &str) -> Value {
    Value::String(s.to_string())
}

pub fn v_num(n: f64) -> Value {
    json!(n)
}

/// Simple profiling scope that emits a structured log on drop.
pub struct ProfileScope {
    module: &'static str,
    label: &'static str,
    started: Instant,
    enabled: bool,
}

impl ProfileScope {
    pub fn new(module: &'static str, label: &'static str) -> Self {
        let enabled = std::env::var("PROFILE_SAMPLE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|p| {
                if p >= 1.0 {
                    true
                } else if p <= 0.0 {
                    false
                } else {
                    let r: f64 = rand::random();
                    r < p
                }
            })
            .unwrap_or(true);
        Self {
            module,
            label,
            started: Instant::now(),
            enabled,
        }
    }

    pub fn with_context(module: &'static str, label: &'static str, fields: &[(&str, Value)]) -> Self {
        let enabled = std::env::var("PROFILE_SAMPLE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|p| {
                if p >= 1.0 {
                    true
                } else if p <= 0.0 {
                    false
                } else {
                    let r: f64 = rand::random();
                    r < p
                }
            })
            .unwrap_or(true);
        if enabled {
            json_log(module, obj(fields));
        }
        Self {
            module,
            label,
            started: Instant::now(),
            enabled,
        }
    }
}

impl Drop for ProfileScope {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        let elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        json_log(
            self.module,
            obj(&[
                ("label", v_str(self.label)),
                ("elapsed_ms", v_num(elapsed_ms)),
            ]),
        );
    }
}
