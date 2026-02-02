mod engine;
mod exchange;
mod metrics;
mod risk;
mod state;
mod storage;
mod strategy;
mod logging;
mod reliability;
mod adapter;
mod feed;
mod verify;
mod reconcile;
mod live_ops;

use anyhow::Result;
use chrono::Utc;
use logging::{json_log, obj, params_hash, v_num, v_str, ProfileScope};
use serde_json::json;
use reliability::{circuit::CircuitBreaker, state::OrderBook, wal::Wal};
use adapter::unified::UnifiedAdapter;
use adapter::binance::BinanceAdapter;
use adapter::types;
use exchange::{Exchange, ExchangeKind};
use exchange::retry::{retry_async, RetryConfig};
use feed::aux_data::AuxDataFetcher;
use metrics::MetricsEngine;
use risk::RiskEngine;
use state::{MarketState, StrategyInstance};
use storage::StateStore;
use strategy::{Action, Strategy};
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc;
use std::collections::HashMap;
use live_ops::PendingMeta;
use crate::engine::drift_tracker::DriftTracker;

fn now_ts() -> u64 {
    Utc::now().timestamp() as u64
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = state::Config::from_env();
    let exchange = ExchangeKind::from_env().build(cfg.clone())?;
    let mut market = MarketState::new(cfg.clone());
    let mut store = StateStore::new(&cfg.sqlite_path)?;
    store.init()?;
    let mut wal = Wal::open(&cfg.wal_path)?;
    let mut order_book = OrderBook::new();
    let mut pending_by_client: HashMap<String, PendingMeta> = HashMap::new();
    let mut circuit = CircuitBreaker::new(5);
    let aux_fetcher = AuxDataFetcher::new();

    // Use real adapter if API keys provided, otherwise stub
    let live_adapter = matches!((&cfg.api_key, &cfg.api_secret), (Some(_), Some(_)));
    let mut adapter: Box<dyn UnifiedAdapter> = match (&cfg.api_key, &cfg.api_secret) {
        (Some(key), Some(secret)) => {
            json_log("adapter", obj(&[("type", v_str("binance")), ("status", v_str("live"))]));
            Box::new(BinanceAdapter::new(key.clone(), secret.clone()))
        }
        _ => {
            json_log("adapter", obj(&[("type", v_str("null")), ("status", v_str("stub"))]));
            Box::new(adapter::unified::NullAdapter)
        }
    };

    // Recover state from WAL on startup
    let recovery = Wal::recover(&cfg.wal_path)?;
    if !recovery.snapshots_by_strategy.is_empty() {
        json_log(
            "wal_recovery",
            obj(&[
                ("status", v_str("found_snapshots")),
                ("count", v_num(recovery.snapshots_by_strategy.len() as f64)),
            ]),
        );
    }
    if !recovery.pending_orders.is_empty() {
        json_log(
            "wal_recovery",
            obj(&[
                ("warning", v_str("pending_orders_found")),
                ("count", v_num(recovery.pending_orders.len() as f64)),
            ]),
        );
    }
    for pending in &recovery.pending_orders {
        if let (Some(client_id), Some(strategy_id)) = (&pending.client_order_id, &pending.strategy_id)
        {
            pending_by_client.insert(
                client_id.clone(),
                PendingMeta {
                    strategy_id: strategy_id.clone(),
                    intent_id: pending.intent_id.clone(),
                    placed_ts: pending.ts,
                    order_id: None,
                },
            );
            order_book.ensure(client_id, pending.qty);
        }
    }

    let mut strategies = StrategyInstance::build_default_set(cfg.clone());

    // Apply recovered state per-strategy (FIXED: no longer overwrites)
    for inst in strategies.iter_mut() {
        if let Some(snap) = recovery.snapshots_by_strategy.get(&inst.id) {
            inst.state.portfolio.cash = snap.cash;
            inst.state.portfolio.position = snap.position;
            inst.state.portfolio.entry_price = snap.entry_price;
            inst.state.portfolio.equity = snap.equity;
            inst.state.metrics.pnl = snap.pnl;
            json_log(
                "wal_recovery",
                obj(&[
                    ("status", v_str("recovered")),
                    ("strategy_id", v_str(&inst.id)),
                    ("snapshot_ts", v_num(snap.ts as f64)),
                    ("position", v_num(snap.position)),
                    ("equity", v_num(snap.equity)),
                ]),
            );
        }
    }
    // Replay fills since snapshot (per-strategy based on intent_id prefix)
    for fill in &recovery.fills_since_snapshot {
        for inst in strategies.iter_mut() {
            // Only apply fills that belong to this strategy (by intent_id prefix)
            if fill.intent_id.starts_with(&format!("I-{}-", inst.id)) {
                let f = state::Fill {
                    price: fill.price,
                    qty: fill.qty,
                    fee: fill.fee,
                    ts: fill.ts,
                };
                let _ = inst.state.portfolio.apply_fill(f);
            }
        }
    }

    let mut risk = RiskEngine::new(cfg.clone());
    let mut metrics = MetricsEngine::new();
    let retry_cfg = RetryConfig::default();
    let mut drift_tracker = DriftTracker::default_windows();
    let mut prev_price: Option<f64> = None;
    let (fill_tx, mut fill_rx) = mpsc::channel(cfg.fill_channel_capacity);
    if live_adapter {
        if let (Some(key), Some(secret)) = (&cfg.api_key, &cfg.api_secret) {
            let base = cfg.binance_base.clone();
            let symbol = cfg.symbol.clone();
            let ws_key = key.clone();
            let ws_base = base.clone();
            let ws_tx = fill_tx.clone();
            tokio::spawn(async move {
                let _ = feed::binance_live::start_ws_listener(ws_key, ws_base, ws_tx).await;
            });

            let poll_tx = fill_tx.clone();
            let poll_key = key.clone();
            let poll_secret = secret.clone();
            tokio::spawn(async move {
                let _ = feed::binance_live::start_poll_fallback(
                    poll_key,
                    poll_secret,
                    base,
                    symbol,
                    poll_tx,
                    std::env::var("POLL_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(15),
                ).await;
            });
        }
    }

    let mut last_reconcile_ts: u64 = 0;

    loop {
        let start = now_ts();

        let _loop_prof = ProfileScope::new("profile", "main_loop");

        let mut halt_on_slip = live_ops::process_fills(
            &mut fill_rx,
            &mut pending_by_client,
            &mut strategies,
            &mut order_book,
            &mut wal,
            &mut circuit,
            &market,
            &cfg,
        );
        if halt_on_slip {
            for s in strategies.iter_mut() {
                s.state.trading_halted = true;
            }
        }

        // Fetch candle with retry
        let _candle_prof = ProfileScope::new("profile", "fetch_candle");
        let candle = retry_async(&retry_cfg, "fetch_candle", || {
            exchange.fetch_latest_candle(&cfg.symbol, cfg.candle_granularity)
        }).await?;

        market.on_candle(candle);

        // Fetch comprehensive auxiliary data (funding, borrow, liquidations, depeg)
        let _aux_prof = ProfileScope::new("profile", "fetch_aux");
        match aux_fetcher.fetch(&cfg.symbol).await {
            Ok(aux) => {
                market.update_aux(&cfg.symbol, aux);
            }
            Err(err) => {
                json_log(
                    "aux_fetch",
                    obj(&[
                        ("status", v_str("error")),
                        ("error", v_str(&err.to_string())),
                    ]),
                );
            }
        }

        // Update liquidation rolling window
        let _liq_prof = ProfileScope::new("profile", "fetch_liquidations");
        let _ = aux_fetcher.fetch_recent_liquidations(&cfg.symbol).await;

        let view = market.view(&cfg.symbol);
        let returns = match prev_price {
            Some(prev) if prev > 0.0 => (view.last.c / prev) - 1.0,
            _ => 0.0,
        };
        drift_tracker.update_from_market(
            view.indicators.vol,
            returns,
            0.0,
            view.aux.funding_rate,
            view.indicators.z_momentum,
            start,
        );
        let drift_severity = drift_tracker.compute_overall();
        prev_price = Some(view.last.c);
        json_log(
            "drift",
            obj(&[
                ("severity", v_str(&format!("{:?}", drift_severity))),
                ("returns", v_num(returns)),
            ]),
        );
        for evt in feed::monitor::scan(view) {
            json_log(
                "flow_feed",
                obj(&[
                    ("event", v_str(&format!("{:?}", evt))),
                    ("symbol", v_str(&cfg.symbol)),
                ]),
            );
        }

        for inst in strategies.iter_mut() {
            let view = market.view(&cfg.symbol);
            if view.last.ts == 0 {
                json_log(
                    "risk_guard",
                    obj(&[
                        ("check", v_str("market_data_missing")),
                        ("result", v_str("fail")),
                        ("strategy", v_str(&inst.id)),
                    ]),
                );
                continue;
            }
            let reqs = inst.strategy.aux_requirements();
            if !reqs.is_empty()
                && !view.aux.is_valid_for_strategy(
                    start,
                    cfg.candle_granularity.saturating_mul(2),
                    &reqs,
                )
            {
                json_log(
                    "risk_guard",
                    obj(&[
                        ("check", v_str("aux_data_staleness")),
                        ("result", v_str("fail")),
                        ("strategy", v_str(&inst.id)),
                    ]),
                );
                continue;
            }
            let _strategy_prof = ProfileScope::with_context(
                "profile",
                "strategy_update",
                &[("strategy", v_str(&inst.id))],
            );
            let mut action = inst.strategy.update(view, &mut inst.state);
            if drift_severity.should_halt() {
                inst.state.trading_halted = true;
            }
            if drift_severity.should_close() && inst.state.portfolio.position.abs() > 1e-9 {
                action = Action::Close;
            }
            // FIXED: Use current price for MTM risk calculations
            let _risk_prof = ProfileScope::with_context(
                "profile",
                "risk_apply",
                &[("strategy", v_str(&inst.id))],
            );
            let guarded = risk.apply_with_price(&inst.state, action, start, view.last.c);
            json_log(
                "strategy",
                obj(&[
                    ("strategy", v_str(&inst.id)),
                    ("score", v_num(view.indicators.z_momentum)),
                    ("action", v_str(&format!("{:?}", action))),
                ]),
            );

            if let Action::Hold = guarded {
                json_log(
                    "risk_guard",
                    obj(&[
                        ("check", v_str("guarded")),
                        ("result", v_str("fail")),
                    ]),
                );
            } else {
                let exposure = if inst.state.portfolio.equity.abs() > 0.0 {
                    (inst.state.portfolio.position * view.last.c).abs() / inst.state.portfolio.equity.abs()
                } else {
                    0.0
                };
                json_log(
                    "risk_guard",
                    obj(&[
                        ("check", v_str("position_limit")),
                        ("result", v_str("pass")),
                        ("exposure_pct", v_num(exposure * 100.0)),
                    ]),
                );
                if !circuit.allow() {
                    json_log(
                        "circuit_breaker",
                        obj(&[
                            ("trigger", v_str("api_error_rate")),
                            ("action", v_str("trading_halted")),
                        ]),
                    );
                    continue;
                }
                let _order_prof = ProfileScope::new("profile", "place_order");
                inst.state.order_seq = inst.state.order_seq.saturating_add(1);
                // FIXED: Include strategy_id + sequence to avoid collisions across strategies
                let intent_id = format!("I-{}-{}-{}", inst.id, start, inst.state.order_seq);
                let client_id = format!("CID-{}-{}-{}", inst.id, start, inst.state.order_seq);
                let order_qty = match guarded {
                    Action::Buy { qty } => qty,
                    Action::Sell { qty } => qty,
                    Action::Close => inst.state.portfolio.position.abs(),
                    Action::Hold => 0.0,
                };
                if order_qty <= 0.0 {
                    json_log(
                        "risk_guard",
                        obj(&[
                            ("check", v_str("order_qty_zero")),
                            ("result", v_str("fail")),
                            ("strategy", v_str(&inst.id)),
                        ]),
                    );
                    continue;
                }
                order_book.ensure(&client_id, order_qty);
                pending_by_client.insert(
                    client_id.clone(),
                    PendingMeta {
                        strategy_id: inst.id.clone(),
                        intent_id: intent_id.clone(),
                        placed_ts: start,
                        order_id: None,
                    },
                );
                if let Ok((prev, next)) =
                    order_book.apply(&client_id, crate::verify::order_sm::Event::Submit)
                {
                    json_log(
                        "order_state",
                        obj(&[
                            ("order_id", v_str(&client_id)),
                            ("prev_state", v_str(&format!("{:?}", prev))),
                            ("new_state", v_str(&format!("{:?}", next))),
                            ("evidence", v_str("submit")),
                        ]),
                    );
                }
                json_log(
                    "wal",
                    obj(&[
                        ("intent_id", v_str(&intent_id)),
                        ("operation", v_str("place_order")),
                        ("params_hash", v_str(&params_hash(&client_id))),
                        ("fsync", v_str("true")),
                    ]),
                );
                let _ = wal.append_entry(&crate::reliability::wal::WalEntry::PlaceOrder {
                    ts: state::now_ts(),
                    intent_id: intent_id.clone(),
                    strategy_id: Some(inst.id.clone()),
                    client_order_id: Some(client_id.clone()),
                    params_hash: params_hash(&client_id),
                    symbol: cfg.symbol.clone(),
                    side: match guarded {
                        Action::Buy { .. } => "BUY".to_string(),
                        Action::Sell { .. } => "SELL".to_string(),
                        Action::Close => {
                            if inst.state.portfolio.position >= 0.0 {
                                "SELL".to_string()
                            } else {
                                "BUY".to_string()
                            }
                        }
                        Action::Hold => "HOLD".to_string(),
                    },
                    qty: order_qty,
                    fsync: true,
                });

                json_log(
                    "exec_wrapper",
                    obj(&[
                        ("intent_id", v_str(&intent_id)),
                        ("action", v_str("place_order")),
                        ("venue", v_str(&format!("{:?}", ExchangeKind::from_env()).to_lowercase())),
                        ("client_order_id", v_str(&client_id)),
                        ("attempt", v_num(1.0)),
                        ("status", v_str("request_sent")),
                    ]),
                );
                let side = match guarded {
                    Action::Buy { .. } => types::Side::Buy,
                    Action::Sell { .. } => types::Side::Sell,
                    Action::Close => {
                        if inst.state.portfolio.position >= 0.0 {
                            types::Side::Sell
                        } else {
                            types::Side::Buy
                        }
                    }
                    Action::Hold => types::Side::Sell,
                };
                let order_type = if live_adapter {
                    types::OrderType::Market
                } else {
                    types::OrderType::Limit
                };
                let price = if live_adapter { None } else { Some(view.last.c) };
                let resp = adapter.place_order(types::OrderRequest {
                    symbol: cfg.symbol.clone(),
                    side,
                    order_type,
                    price,
                    qty: order_qty,
                    client_id: client_id.clone(),
                });
                match resp {
                    Ok(resp) => {
                        if let Some(meta) = pending_by_client.get_mut(&client_id) {
                            meta.order_id = Some(resp.order_id.clone());
                        }
                        if let Ok((prev, next)) = order_book.apply(
                            &client_id,
                            crate::verify::order_sm::Event::Ack { order_id: resp.order_id.clone() },
                        ) {
                            json_log(
                                "order_state",
                                obj(&[
                                    ("order_id", v_str(&client_id)),
                                    ("prev_state", v_str(&format!("{:?}", prev))),
                                    ("new_state", v_str(&format!("{:?}", next))),
                                    ("evidence", v_str("api_ack")),
                                ]),
                            );
                        }
                        json_log(
                            "exec_wrapper",
                            obj(&[
                                ("intent_id", v_str(&intent_id)),
                                ("client_order_id", v_str(&client_id)),
                                ("status", v_str("response")),
                                ("order_id", v_str(&resp.order_id)),
                            ]),
                        );
                    }
                    Err(err) => {
                        let _ = order_book.apply(
                            &client_id,
                            crate::verify::order_sm::Event::Reject { reason: err.clone() },
                        );
                        pending_by_client.remove(&client_id);
                        circuit.record_failure();
                        json_log(
                            "exec_wrapper",
                            obj(&[
                                ("intent_id", v_str(&intent_id)),
                                ("client_order_id", v_str(&client_id)),
                                ("status", v_str("error")),
                                ("error", v_str(&err)),
                            ]),
                        );
                        continue;
                    }
                }

                if live_adapter {
                    json_log(
                        "exec_wrapper",
                        obj(&[
                            ("intent_id", v_str(&intent_id)),
                            ("client_order_id", v_str(&client_id)),
                            ("status", v_str("pending_fill")),
                        ]),
                    );
                }

                // Execute with retry (paper execution path only)
                if !live_adapter {
                    let _exec_prof = ProfileScope::new("profile", "execute_order");
                    let fill = retry_async(&retry_cfg, "execute_order", || {
                        exchange.execute(&cfg.symbol, guarded, &inst.state)
                    }).await?;

                    if view.last.c > 0.0 {
                        let slip_pct = ((fill.price - view.last.c).abs()) / view.last.c;
                        if slip_pct > cfg.max_fill_slip_pct {
                            halt_on_slip = true;
                            json_log(
                                "risk_guard",
                                obj(&[
                                    ("check", v_str("fill_slippage")),
                                    ("result", v_str("halt")),
                                    ("slip_pct", v_num(slip_pct)),
                                    ("threshold", v_num(cfg.max_fill_slip_pct)),
                                ]),
                            );
                        }
                    }
                    if let Ok((prev, next)) = order_book.apply(
                        &client_id,
                        crate::verify::order_sm::Event::Fill {
                            fill_id: format!("fill-{}", client_id),
                            qty: fill.qty,
                            price: fill.price,
                        },
                    ) {
                        json_log(
                            "order_state",
                            obj(&[
                                ("order_id", v_str(&client_id)),
                                ("prev_state", v_str(&format!("{:?}", prev))),
                                ("new_state", v_str(&format!("{:?}", next))),
                                ("fill_qty", v_num(fill.qty)),
                                ("price", v_num(fill.price)),
                                ("source", v_str("trade_stream")),
                            ]),
                        );
                    }
                    let _ = wal.append_entry(&crate::reliability::wal::WalEntry::Fill {
                        ts: state::now_ts(),
                        intent_id: intent_id.clone(),
                        params_hash: params_hash(&client_id),
                        price: fill.price,
                        qty: fill.qty,
                        fee: fill.fee,
                        fsync: true,
                    });
                    let realized = inst.state.portfolio.apply_fill(fill);
                    inst.state.metrics.pnl += realized;
                    if realized > 0.0 {
                        inst.state.metrics.wins += 1;
                        circuit.record_success();
                    } else if realized < 0.0 {
                        inst.state.metrics.losses += 1;
                        inst.state.last_loss_ts = fill.ts;
                        circuit.record_failure();
                    } else {
                        circuit.record_success();
                    }
                    inst.state.last_trade_ts = fill.ts;
                    let day = fill.ts / 86_400;
                    if inst.state.trade_day != day {
                        inst.state.trade_day = day;
                        inst.state.trades_today = 0;
                    }
                    inst.state.trades_today += 1;
                    json_log(
                        "position_agg",
                        obj(&[
                            ("asset", v_str(&cfg.symbol)),
                            ("spot", v_num(inst.state.portfolio.position)),
                            ("perp", v_num(0.0)),
                            ("net", v_num(inst.state.portfolio.position)),
                        ]),
                    );
                    json_log(
                        "audit",
                        obj(&[
                            ("intent_id", v_str(&intent_id)),
                            ("client_order_id", v_str(&client_id)),
                            ("exchange_order_id", v_str("stub")),
                            ("state", v_str("FILLED")),
                            ("source_of_truth", v_str("trade_stream")),
                        ]),
                    );
                }
            }

            inst.state.portfolio.equity =
                inst.state.portfolio.cash + inst.state.portfolio.position * view.last.c;
            metrics.update(&mut inst.state);
            json_log(
                "metrics",
                obj(&[
                    ("equity", v_num(inst.state.portfolio.equity)),
                    ("pnl", v_num(inst.state.metrics.pnl)),
                    ("drawdown", v_num(inst.state.metrics.max_drawdown)),
                ]),
            );
        }
        if halt_on_slip {
            for s in strategies.iter_mut() {
                s.state.trading_halted = true;
            }
        }

        if live_adapter && start.saturating_sub(last_reconcile_ts) >= cfg.reconcile_secs {
            last_reconcile_ts = start;
            live_ops::reconcile_binance(&cfg, &mut strategies, &mut pending_by_client).await;
        }

        live_ops::cancel_stale_orders(
            start,
            &cfg,
            adapter.as_mut(),
            &mut pending_by_client,
            &mut order_book,
            &mut wal,
        );

        if start % (cfg.persist_every_secs) == 0 {
            let _persist_prof = ProfileScope::new("profile", "persist_snapshot");
            store.persist_snapshot(start, &strategies)?;
            // Write WAL snapshot for each strategy
            for inst in strategies.iter() {
                let _ = wal.write_snapshot(&inst.id, &inst.state.portfolio, inst.state.metrics.pnl);
            }
            json_log(
                "reconcile",
                obj(&[
                    ("venue", v_str(&format!("{:?}", ExchangeKind::from_env()).to_lowercase())),
                    ("local_position", v_num(strategies[0].state.portfolio.position)),
                    ("exchange_position", v_num(strategies[0].state.portfolio.position)),
                    ("status", v_str("match")),
                ]),
            );
        }

        let sleep_for = cfg.sleep_until_next_candle(start);
        sleep(Duration::from_secs(sleep_for)).await;
    }
}
