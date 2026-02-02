//! Epistemic Dashboard Server
//!
//! Serves the epistemic state of the system as JSON for the ClojureScript dashboard.
//! Run with: cargo run --bin epistemic_server

use arbitragefx::epistemic::EpistemicState;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn main() {
    let port = 8765;
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .expect("Failed to bind");

    println!("Epistemic Server running at http://localhost:{}", port);
    println!();
    println!("Endpoints:");
    println!("  GET /api/state  - Full epistemic state as JSON");
    println!("  GET /api/health - Health check");
    println!();

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let buf_reader = BufReader::new(&stream);
        let request_line = buf_reader.lines().next();

        let request = match request_line {
            Some(Ok(line)) => line,
            _ => continue,
        };

        let (status, content_type, body) = if request.starts_with("GET /api/state") {
            let state = EpistemicState::from_system();
            ("200 OK", "application/json", state.to_json())
        } else if request.starts_with("GET /api/health") {
            ("200 OK", "application/json", r#"{"status":"ok"}"#.to_string())
        } else if request.starts_with("GET /api/summary") {
            let state = EpistemicState::from_system();
            let counts = state.count_by_level();
            let summary = serde_json::json!({
                "hypotheses": state.hypotheses.len(),
                "signals": state.signals.len(),
                "filters": state.filters.len(),
                "invariants_hold": state.invariants_hold(),
                "by_level": counts.iter()
                    .map(|(k, v)| (format!("{:?}", k), v))
                    .collect::<std::collections::HashMap<_, _>>()
            });
            ("200 OK", "application/json", summary.to_string())
        } else {
            ("404 NOT FOUND", "text/plain", "Not Found".to_string())
        };

        let response = format!(
            "HTTP/1.1 {}\r\n\
             Content-Type: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Content-Length: {}\r\n\r\n{}",
            status,
            content_type,
            body.len(),
            body
        );

        let _ = stream.write_all(response.as_bytes());
    }
}
