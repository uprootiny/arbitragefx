use anyhow::Result;
use tokio::time::{sleep, Duration};

use arbitragefx::exchange::{Exchange, ExchangeKind};
use arbitragefx::feed::aux_data::AuxDataFetcher;
use arbitragefx::metrics::MetricsEngine;
use arbitragefx::risk::RiskEngine;
use arbitragefx::state::{Config, MarketState, StrategyInstance};
use arbitragefx::strategy::Action;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::from_env();
    let exchange = ExchangeKind::from_env().build(cfg.clone())?;
    let aux_fetcher = AuxDataFetcher::new();
    let mut market = MarketState::new(cfg.clone());
    let mut strategies = StrategyInstance::build_carry_event_set(cfg.clone());
    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();

    eprintln!("[live_loop] starting with symbol={}", cfg.symbol);

    loop {
        let candle = exchange
            .fetch_latest_candle(&cfg.symbol, cfg.candle_granularity)
            .await?;
        market.on_candle(candle);

        // Fetch real auxiliary data (funding, borrow, liquidations, depeg)
        let aux = aux_fetcher.fetch(&cfg.symbol).await.unwrap_or_default();
        market.update_aux(&cfg.symbol, aux);

        // Also fetch recent liquidations to update the rolling window
        let _ = aux_fetcher.fetch_recent_liquidations(&cfg.symbol).await;

        for inst in strategies.iter_mut() {
            let view = market.view(&cfg.symbol);
            let action = inst.strategy.update(view, &mut inst.state);
            // Use current price for MTM risk calculations
            let guarded = risk.apply_with_price(&inst.state, action, candle.ts, view.last.c);

            eprintln!(
                "[{}] price={:.2} funding={:.6} liq_score={:.2} action={:?}",
                inst.id, view.last.c, aux.funding_rate, aux.liquidation_score, action
            );

            if let Action::Hold = guarded {
                // risk guard blocked
            } else {
                let fill = exchange.execute(&cfg.symbol, guarded, &inst.state).await?;
                let realized = inst.state.portfolio.apply_fill(fill);
                inst.state.metrics.pnl += realized;

                eprintln!(
                    "[{}] FILL price={:.2} qty={:.6} pnl={:.4}",
                    inst.id, fill.price, fill.qty, realized
                );
            }
            metrics.update(&mut inst.state);
        }

        let sleep_for = cfg.sleep_until_next_candle(candle.ts);
        sleep(Duration::from_secs(sleep_for)).await;
    }
}
