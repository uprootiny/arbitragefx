use arbitragefx::skeleton::engine::{run_loop, EngineConfig};
use arbitragefx::skeleton::exec::PaperExec;
use arbitragefx::skeleton::log::Logger;
use arbitragefx::skeleton::state::{Candle, EngineState};
use arbitragefx::skeleton::strategy::SimpleMomentum;
use arbitragefx::skeleton::wal::Wal;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let mut state = EngineState::new(10, 10_000.0);
    let mut strat = SimpleMomentum { qty: 0.01, threshold: 0.002 };
    let mut exec = PaperExec { fee_rate: 0.001, slip_rate: 0.0005 };
    let mut wal = Wal::open(Path::new("out/skeleton/bot.wal"))?;
    let mut logger = Logger::new(Path::new("out/skeleton/runs"), "skeleton-run".to_string())?;
    let cfg = EngineConfig { sleep_secs: 60, max_position: 1_000.0 };

    let candles = (0..50).map(|i| Candle {
        ts: i,
        o: 100.0 + i as f64,
        h: 101.0 + i as f64,
        l: 99.0 + i as f64,
        c: 100.0 + i as f64,
        v: 1.0,
    });

    run_loop(&mut state, &mut strat, &mut exec, &mut wal, &mut logger, &cfg, candles);
    Ok(())
}
