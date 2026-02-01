use chrono::Utc;
use serde_json::{json, Map, Value};

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
