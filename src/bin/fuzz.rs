use std::env;

fn lcg(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed
}

fn pick(seed: &mut u64, n: u64) -> u64 {
    lcg(seed) % n
}

fn main() {
    let mut seed = env::var("SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(42u64);
    let events = env::var("EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50u64);

    println!(r#"{{"type":"Submit"}}"#);
    let mut order_id_emitted = false;
    let mut fill_count = 0u64;

    for _ in 0..events {
        let roll = pick(&mut seed, 100);
        if roll < 15 && !order_id_emitted {
            println!(
                r#"{{"type":"Ack","order_id":"oid-{}"}}"#,
                pick(&mut seed, 9999)
            );
            order_id_emitted = true;
        } else if roll < 60 {
            let dup = pick(&mut seed, 10) < 2;
            let fid = if dup && fill_count > 0 {
                format!("fill-{}", fill_count - 1)
            } else {
                let f = format!("fill-{}", fill_count);
                fill_count += 1;
                f
            };
            let qty = 0.1;
            let price = 100.0 + (pick(&mut seed, 100) as f64) * 0.1;
            let ts = pick(&mut seed, 10_000);
            println!(
                r#"{{"type":"Fill","fill_id":"{}","qty":{},"price":{},"ts":{}}}"#,
                fid, qty, price, ts
            );
        } else if roll < 75 {
            println!(r#"{{"type":"CancelRequest"}}"#);
        } else if roll < 85 {
            println!(r#"{{"type":"CancelAck"}}"#);
        } else if roll < 92 {
            println!(r#"{{"type":"Timeout"}}"#);
        } else {
            println!(r#"{{"type":"Reject","reason":"simulated"}}"#);
        }
    }
}
