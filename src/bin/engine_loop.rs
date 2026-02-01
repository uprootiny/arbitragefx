//! Event-driven trading loop using the new engine architecture.

use anyhow::Result;
use tokio::time::{interval, Duration};

use arbitragefx::engine::{
    bus::EventBus,
    events::*,
    reducer::{reduce, ReducerConfig, ReducerOutput},
    state::EngineState,
};
use arbitragefx::exchange::{Exchange, ExchangeKind};
use arbitragefx::feed::aux_data::AuxDataFetcher;
use arbitragefx::state::Config;

struct Runner {
    state: EngineState,
    bus: EventBus,
    cfg: ReducerConfig,
    exchange: Box<dyn Exchange + Send + Sync>,
    aux_fetcher: AuxDataFetcher,
    symbol: String,
    candle_interval_secs: u64,
}

impl Runner {
    fn new(symbol: String, candle_interval_secs: u64) -> Result<Self> {
        let config = Config::from_env();
        let exchange = ExchangeKind::from_env().build(config.clone())?;

        Ok(Self {
            state: EngineState::new(),
            bus: EventBus::new(),
            cfg: ReducerConfig::default(),
            exchange,
            aux_fetcher: AuxDataFetcher::new(),
            symbol,
            candle_interval_secs,
        })
    }

    /// Fetch market data and push events onto bus
    async fn ingest(&mut self) -> Result<()> {
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        // Fetch candle
        let candle = self.exchange
            .fetch_latest_candle(&self.symbol, self.candle_interval_secs)
            .await?;

        self.bus.push(Event::Market(MarketEvent::Candle {
            ts: candle.ts * 1000, // convert to ms
            symbol: self.symbol.clone(),
            o: candle.o,
            h: candle.h,
            l: candle.l,
            c: candle.c,
            v: candle.v,
        }));

        // Fetch aux data
        let aux = self.aux_fetcher.fetch(&self.symbol).await.unwrap_or_default();

        if aux.funding_rate != 0.0 {
            self.bus.push(Event::Market(MarketEvent::Funding {
                ts: now_ms,
                symbol: self.symbol.clone(),
                rate: aux.funding_rate,
                next_ts: now_ms + 8 * 3600 * 1000, // ~8 hours
            }));
        }

        // Fetch liquidations
        let _ = self.aux_fetcher.fetch_recent_liquidations(&self.symbol).await;

        // Timer event for housekeeping
        self.bus.push(Event::Sys(SysEvent::Timer {
            ts: now_ms,
            name: "tick".to_string(),
        }));

        Ok(())
    }

    /// Process all queued events through reducer
    fn process(&mut self) -> Vec<Command> {
        let mut all_commands = Vec::new();

        while let Some(event) = self.bus.pop() {
            let output: ReducerOutput = reduce(&mut self.state, event, &self.cfg);
            all_commands.extend(output.commands);
        }

        all_commands
    }

    /// Execute commands against exchange
    async fn execute(&mut self, commands: Vec<Command>) -> Result<()> {
        for cmd in commands {
            match cmd {
                Command::PlaceOrder { symbol, client_id, side, qty, price } => {
                    eprintln!(
                        "[EXEC] {} {} {:.6} @ {:?} id={}",
                        symbol,
                        if matches!(side, TradeSide::Buy) { "BUY" } else { "SELL" },
                        qty,
                        price,
                        client_id
                    );

                    // Convert to strategy Action and execute
                    let action = match side {
                        TradeSide::Buy => arbitragefx::strategy::Action::Buy { qty },
                        TradeSide::Sell => arbitragefx::strategy::Action::Sell { qty },
                    };

                    // Create minimal strategy state for execution
                    let strategy_state = arbitragefx::strategy::StrategyState {
                        portfolio: arbitragefx::strategy::PortfolioState {
                            cash: self.state.portfolio.cash,
                            position: self.state.portfolio.positions
                                .get(&symbol)
                                .map(|p| p.qty)
                                .unwrap_or(0.0),
                            entry_price: self.state.portfolio.positions
                                .get(&symbol)
                                .map(|p| p.entry_price)
                                .unwrap_or(0.0),
                            equity: self.state.portfolio.equity,
                        },
                        metrics: arbitragefx::strategy::MetricsState::default(),
                        last_trade_ts: self.state.risk.last_trade_ts,
                        last_loss_ts: self.state.risk.last_loss_ts,
                        trading_halted: self.state.halted,
                        trades_today: self.state.risk.trades_today,
                        trade_day: self.state.risk.trade_day,
                        order_seq: self.state.seq,
                    };

                    match self.exchange.execute(&symbol, action, &strategy_state).await {
                        Ok(fill) => {
                            if fill.qty != 0.0 {
                                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                                self.bus.push(Event::Exec(ExecEvent::Fill {
                                    ts: now_ms,
                                    symbol: symbol.clone(),
                                    client_id: client_id.clone(),
                                    order_id: format!("ex-{}", now_ms),
                                    fill_id: format!("fill-{}", now_ms),
                                    price: fill.price,
                                    qty: fill.qty.abs(),
                                    fee: fill.fee,
                                    side,
                                }));
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] execution failed: {}", e);
                            let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                            self.bus.push(Event::Exec(ExecEvent::Reject {
                                ts: now_ms,
                                symbol,
                                client_id,
                                reason: e.to_string(),
                            }));
                        }
                    }
                }

                Command::CancelOrder { symbol, client_id } => {
                    eprintln!("[CANCEL] {} {}", symbol, client_id);
                    // TODO: implement cancel via adapter
                }

                Command::CancelAll { symbol } => {
                    eprintln!("[CANCEL_ALL] {:?}", symbol);
                    // TODO: implement cancel all
                }

                Command::Halt { reason } => {
                    eprintln!("[HALT] {:?}", reason);
                    self.state.halted = true;
                }

                Command::Log { level, msg } => {
                    match level {
                        LogLevel::Debug => eprintln!("[DEBUG] {}", msg),
                        LogLevel::Info => eprintln!("[INFO] {}", msg),
                        LogLevel::Warn => eprintln!("[WARN] {}", msg),
                        LogLevel::Error => eprintln!("[ERROR] {}", msg),
                    }
                }
            }
        }

        Ok(())
    }

    /// Print current state summary
    fn status(&self) {
        eprintln!(
            "[STATE] seq={} halted={} cash={:.2} equity={:.2} dd={:.2}% trades={}",
            self.state.seq,
            self.state.halted,
            self.state.portfolio.cash,
            self.state.portfolio.equity,
            self.state.portfolio.drawdown_pct() * 100.0,
            self.state.risk.trades_today,
        );

        for (sym, pos) in &self.state.portfolio.positions {
            if pos.qty != 0.0 {
                eprintln!("  [POS] {} qty={:.6} entry={:.2}", sym, pos.qty, pos.entry_price);
            }
        }

        if let Some(sym_state) = self.state.symbols.get(&self.symbol) {
            eprintln!(
                "  [MKT] {} price={:.2} z_mom={:.3} vol={:.4} funding={:.6}",
                self.symbol,
                sym_state.last_price,
                sym_state.z_momentum(),
                sym_state.volatility,
                sym_state.funding_rate,
            );
        }

        eprintln!("  [HASH] {}", self.state.hash());
    }

    /// Main loop
    async fn run(&mut self) -> Result<()> {
        eprintln!("[START] symbol={} interval={}s", self.symbol, self.candle_interval_secs);

        let mut ticker = interval(Duration::from_secs(self.candle_interval_secs));

        loop {
            ticker.tick().await;

            if self.state.halted {
                eprintln!("[HALTED] reason={:?}", self.state.halt_reason);
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            }

            // Ingest -> Process -> Execute cycle
            if let Err(e) = self.ingest().await {
                eprintln!("[ERROR] ingest failed: {}", e);
                self.state.risk.consecutive_errors += 1;
                continue;
            }

            let commands = self.process();

            if let Err(e) = self.execute(commands).await {
                eprintln!("[ERROR] execute failed: {}", e);
            }

            // Process any events generated by execution (fills, rejects)
            let followup = self.process();
            if !followup.is_empty() {
                if let Err(e) = self.execute(followup).await {
                    eprintln!("[ERROR] followup failed: {}", e);
                }
            }

            self.status();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let symbol = std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string());
    let interval = std::env::var("INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60u64);

    let mut runner = Runner::new(symbol, interval)?;
    runner.run().await
}
