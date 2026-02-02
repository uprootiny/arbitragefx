use std::collections::HashMap;

use crate::adapter::unified::UnifiedAdapter;
use crate::logging::{json_log, obj, params_hash, v_num, v_str};
use crate::state::MarketState;
use crate::reliability::circuit::CircuitBreaker;
use crate::reconcile::binance::BinanceReconcileClient;
use crate::reliability::{state::OrderBook, wal::Wal};
use crate::state::{Config, StrategyInstance};
use crate::verify::order_sm::{Event, OrderState};
use tokio::sync::mpsc;
use crate::feed::binance_live::FillEvent;

#[derive(Debug, Clone)]
pub struct PendingMeta {
    pub strategy_id: String,
    pub intent_id: String,
    pub placed_ts: u64,
    pub order_id: Option<String>,
}

pub fn process_fills(
    fill_rx: &mut mpsc::Receiver<FillEvent>,
    pending_by_client: &mut HashMap<String, PendingMeta>,
    strategies: &mut [StrategyInstance],
    order_book: &mut OrderBook,
    wal: &mut Wal,
    circuit: &mut CircuitBreaker,
    market: &MarketState,
    cfg: &Config,
) -> bool {
    let mut halt_on_slip = false;
    while let Ok(fill) = fill_rx.try_recv() {
        if let Some(meta) = pending_by_client.get(&fill.client_id).cloned() {
            if let Some(inst) = strategies.iter_mut().find(|s| s.id == meta.strategy_id) {
                let last_price = market.view(&cfg.symbol).last.c;
                if last_price > 0.0 {
                    let slip_pct = ((fill.price - last_price).abs()) / last_price;
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
                    &fill.client_id,
                    Event::Fill {
                        fill_id: fill.fill_id.clone(),
                        qty: fill.qty,
                        price: fill.price,
                    },
                ) {
                    json_log(
                        "order_state",
                        obj(&[
                            ("order_id", v_str(&fill.client_id)),
                            ("prev_state", v_str(&format!("{:?}", prev))),
                            ("new_state", v_str(&format!("{:?}", next))),
                            ("fill_qty", v_num(fill.qty)),
                            ("price", v_num(fill.price)),
                            ("source", v_str("live_fill")),
                        ]),
                    );
                    if next == OrderState::Filled {
                        pending_by_client.remove(&fill.client_id);
                    }
                }

                let signed_qty = if fill.side == "BUY" { fill.qty } else { -fill.qty };
                let realized = inst.state.portfolio.apply_fill(crate::state::Fill {
                    price: fill.price,
                    qty: signed_qty,
                    fee: fill.fee,
                    ts: fill.ts,
                });
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
                let _ = wal.append_entry(&crate::reliability::wal::WalEntry::Fill {
                    ts: crate::state::now_ts(),
                    intent_id: meta.intent_id,
                    params_hash: params_hash(&fill.client_id),
                    price: fill.price,
                    qty: signed_qty,
                    fee: fill.fee,
                    fsync: true,
                });
            }
        } else {
            json_log(
                "fill_unmatched",
                obj(&[
                    ("client_order_id", v_str(&fill.client_id)),
                    ("status", v_str("no_strategy_mapping")),
                ]),
            );
        }
    }
    halt_on_slip
}

pub async fn reconcile_binance(
    cfg: &Config,
    strategies: &mut [StrategyInstance],
    pending_by_client: &mut HashMap<String, PendingMeta>,
) {
    let (Some(key), Some(secret)) = (&cfg.api_key, &cfg.api_secret) else {
        return;
    };
    let client = BinanceReconcileClient::new(
        cfg.binance_base.clone(),
        cfg.binance_fapi_base.clone(),
        key.clone(),
        secret.clone(),
    );

    match client.fetch_open_orders(&cfg.symbol).await {
        Ok(orders) => {
            let mut open_clients = HashMap::new();
            for o in &orders {
                open_clients.insert(o.client_order_id.clone(), o.order_id.clone());
            }
            for (client_id, meta) in pending_by_client.iter_mut() {
                if let Some(order_id) = open_clients.get(client_id) {
                    meta.order_id = Some(order_id.clone());
                }
            }
            json_log(
                "reconcile",
                obj(&[
                    ("venue", v_str("binance")),
                    ("open_orders", v_num(orders.len() as f64)),
                    ("status", v_str("ok")),
                ]),
            );
        }
        Err(err) => {
            json_log(
                "reconcile",
                obj(&[
                    ("venue", v_str("binance")),
                    ("status", v_str("error")),
                    ("error", v_str(&err.to_string())),
                ]),
            );
        }
    }

    match client.fetch_spot_balances().await {
        Ok(balances) => {
            let mut quote_balance = None;
            let mut base_balance = None;
            for b in balances {
                if cfg.symbol.ends_with(&b.asset) {
                    quote_balance = Some(b.free + b.locked);
                }
                if cfg.symbol.starts_with(&b.asset) {
                    base_balance = Some(b.free + b.locked);
                }
            }
            if let (Some(q), Some(b)) = (quote_balance, base_balance) {
                let local_pos: f64 = strategies.iter().map(|s| s.state.portfolio.position).sum();
                let drift = (local_pos - b).abs();
                let thresh = (local_pos.abs() * cfg.reconcile_drift_pct).max(cfg.reconcile_drift_abs);
                if drift > thresh {
                    for inst in strategies.iter_mut() {
                        inst.state.trading_halted = true;
                    }
                    json_log(
                        "reconcile",
                        obj(&[
                            ("venue", v_str("binance")),
                            ("status", v_str("drift_halt")),
                            ("local_pos", v_num(local_pos)),
                            ("exchange_pos", v_num(b)),
                            ("drift", v_num(drift)),
                            ("threshold", v_num(thresh)),
                        ]),
                    );
                }
                json_log(
                    "reconcile",
                    obj(&[
                        ("venue", v_str("binance")),
                        ("base_balance", v_num(b)),
                        ("quote_balance", v_num(q)),
                        ("status", v_str("balances")),
                    ]),
                );
            }
        }
        Err(err) => {
            json_log(
                "reconcile",
                obj(&[
                    ("venue", v_str("binance")),
                    ("status", v_str("balance_error")),
                    ("error", v_str(&err.to_string())),
                ]),
            );
        }
    }

    match client.fetch_futures_positions(&cfg.symbol).await {
        Ok(positions) => {
            if let Some(pos) = positions.first() {
                json_log(
                    "reconcile",
                    obj(&[
                        ("venue", v_str("binance")),
                        ("perp_symbol", v_str(&pos.symbol)),
                        ("perp_pos", v_num(pos.position_amt)),
                        ("perp_entry", v_num(pos.entry_price)),
                        ("perp_mark", v_num(pos.mark_price)),
                        ("status", v_str("perp_position")),
                    ]),
                );
            }
        }
        Err(err) => {
            json_log(
                "reconcile",
                obj(&[
                    ("venue", v_str("binance")),
                    ("status", v_str("perp_error")),
                    ("error", v_str(&err.to_string())),
                ]),
            );
        }
    }
}

pub fn cancel_stale_orders(
    start: u64,
    cfg: &Config,
    adapter: &mut dyn UnifiedAdapter,
    pending_by_client: &mut HashMap<String, PendingMeta>,
    order_book: &mut OrderBook,
    wal: &mut Wal,
) {
    let cancel_after = cfg.cancel_after_candles.saturating_mul(cfg.candle_granularity);
    if cancel_after == 0 {
        return;
    }
    let mut to_cancel: Vec<(String, String)> = Vec::new();
    for (client_id, meta) in pending_by_client.iter() {
        if start.saturating_sub(meta.placed_ts) >= cancel_after {
            if let Some(order_id) = &meta.order_id {
                to_cancel.push((client_id.clone(), order_id.clone()));
            }
        }
    }
    for (client_id, order_id) in to_cancel {
        if adapter.cancel_order(&order_id).is_ok() {
            pending_by_client.remove(&client_id);
            let _ = order_book.apply(&client_id, Event::CancelRequest);
            let _ = wal.append_entry(&crate::reliability::wal::WalEntry::Cancel {
                ts: crate::logging::ts_epoch_ms(),
                intent_id: format!("cancel-{}", client_id),
                params_hash: params_hash(&client_id),
                fsync: true,
            });
            json_log(
                "exec_wrapper",
                obj(&[
                    ("client_order_id", v_str(&client_id)),
                    ("status", v_str("cancelled_timeout")),
                ]),
            );
            let _ = order_book.apply(&client_id, Event::CancelAck);
        }
    }
}
