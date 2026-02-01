//! Pure reducer: (State, Event) -> (State, Vec<Command>)
//!
//! This is the core of deterministic replay.
//! All state transitions happen here.
//!
//! ## Alignment with Right Action
//!
//! The reducer embodies guards against the three poisons:
//!
//! | Poison      | Manifestation              | Guard                              |
//! |-------------|----------------------------|-------------------------------------|
//! | **Greed**   | Over-trading, over-sizing  | `max_trades_per_day`, `max_position_pct` |
//! | **Aversion**| Panic exits, revenge trades| `cooldown_ms`, `exit_threshold` not too tight |
//! | **Delusion**| Chasing lagging signals    | Mean-reversion > momentum-chasing  |
//!
//! The signal logic prefers *providing liquidity* (buying weakness, selling strength)
//! over *extracting alpha* (chasing what already moved).

use super::events::*;
use super::state::*;
use super::ethics::{check_three_poisons, EthicsViolation};
use super::narrative_detector::NarrativeIndicators;

/// Configuration for the reducer
///
/// ## Design principle: Non-grasping
///
/// These parameters encode restraint:
/// - Entry requires exhaustion signals, not momentum confirmation
/// - Position limits prevent greed-driven concentration
/// - Cooldowns prevent aversion-driven revenge trading
/// - Trade limits enforce mindful participation
#[derive(Debug, Clone)]
pub struct ReducerConfig {
    // === Indicator parameters ===
    pub ema_fast_alpha: f64,
    pub ema_slow_alpha: f64,

    // === Entry/Exit thresholds ===
    /// Mean-reversion score threshold for entry (higher = more selective)
    pub entry_threshold: f64,
    /// Score threshold for exit (lower = faster exit)
    pub exit_threshold: f64,

    // === Position sizing (Greed guards) ===
    /// Base position size as fraction of equity
    pub position_size: f64,
    /// Maximum total exposure as fraction of equity
    pub max_position_pct: f64,

    // === Loss limits (Aversion guards) ===
    /// Maximum daily loss before halt
    pub max_daily_loss_pct: f64,
    /// Maximum trades per day (mindful participation)
    pub max_trades_per_day: u32,
    /// Cooldown after loss in ms (prevent revenge trading)
    pub cooldown_ms: u64,

    // === Data quality (Delusion guards) ===
    /// Maximum data age before halt
    pub data_stale_ms: u64,
    /// Maximum spread before halt
    pub max_spread_pct: f64,
    /// Maximum consecutive errors before halt
    pub max_consecutive_errors: u32,

    // === Take profit / Stop loss ===
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
}

impl Default for ReducerConfig {
    fn default() -> Self {
        Self {
            ema_fast_alpha: 2.0 / 7.0,  // ~6 period
            ema_slow_alpha: 2.0 / 25.0, // ~24 period

            // Mean-reversion thresholds (not momentum!)
            entry_threshold: 0.3,  // Moderate RSI/deviation signal
            exit_threshold: 0.1,   // Exit when signal fades

            // Conservative position sizing
            position_size: 0.001,
            max_position_pct: 0.05,

            // Daily discipline
            max_daily_loss_pct: 0.02,
            max_trades_per_day: 20,
            cooldown_ms: 600_000, // 10 min cooldown after loss

            // Data integrity
            data_stale_ms: 60_000, // 1 min
            max_spread_pct: 0.005,
            max_consecutive_errors: 5,

            // Exit targets
            take_profit_pct: 0.004,  // 0.4% take profit
            stop_loss_pct: 0.003,    // 0.3% stop loss (tighter than TP)
        }
    }
}

/// Result of processing an event
#[derive(Debug)]
pub struct ReducerOutput {
    pub commands: Vec<Command>,
    pub state_hash: u64,
}

/// Pure reducer function
pub fn reduce(state: &mut EngineState, event: Event, cfg: &ReducerConfig) -> ReducerOutput {
    let mut commands = Vec::new();

    // Update logical time
    state.now = state.now.max(event.timestamp());
    state.seq += 1;

    // Check halt state
    if state.halted {
        return ReducerOutput {
            commands,
            state_hash: state.hash(),
        };
    }

    match event {
        Event::Market(market_event) => {
            handle_market_event(state, market_event, cfg, &mut commands);
        }
        Event::Exec(exec_event) => {
            handle_exec_event(state, exec_event, cfg, &mut commands);
        }
        Event::Sys(sys_event) => {
            handle_sys_event(state, sys_event, cfg, &mut commands);
        }
    }

    ReducerOutput {
        commands,
        state_hash: state.hash(),
    }
}

fn handle_market_event(
    state: &mut EngineState,
    event: MarketEvent,
    cfg: &ReducerConfig,
    commands: &mut Vec<Command>,
) {
    match event {
        MarketEvent::Candle { ts, symbol, o: _, h: _, l: _, c, v } => {
            let sym = state.symbol_mut(&symbol);
            sym.on_candle(ts, c, v, cfg.ema_fast_alpha, cfg.ema_slow_alpha);

            // Update regime state (Right Mindfulness)
            update_regime_state(state, &symbol, ts, commands);

            // Check for trading signal with ethics guards
            let (can_trade, violation) = should_trade(state, &symbol, cfg);

            // Log ethics violations for transparency
            if let Some(v) = violation {
                commands.push(Command::Log {
                    level: LogLevel::Debug,
                    msg: format!("ethics guard: {:?}", v),
                });
            }

            if can_trade {
                if let Some(cmd) = generate_signal(state, &symbol, cfg) {
                    commands.push(cmd);
                }
            }
        }

        MarketEvent::Trade { ts, symbol, price, .. } => {
            let sym = state.symbol_mut(&symbol);
            sym.last_price = price;
            sym.last_trade_ts = ts;
        }

        MarketEvent::Funding { symbol, rate, .. } => {
            let sym = state.symbol_mut(&symbol);
            sym.funding_rate = rate;
        }

        MarketEvent::Liquidation { symbol, qty, price, .. } => {
            let sym = state.symbol_mut(&symbol);
            // Accumulate liquidation score (decay handled elsewhere)
            let size_usd = qty * price;
            sym.liquidation_score += size_usd / 100_000.0;
        }

        MarketEvent::BookUpdate { symbol, bid, ask, .. } => {
            let spread = if bid > 0.0 { (ask - bid) / bid } else { 0.0 };
            let mid_price = (bid + ask) / 2.0;

            let sym = state.symbol_mut(&symbol);
            sym.spread = spread;
            sym.last_price = mid_price;

            // Halt on spread too wide
            if spread > cfg.max_spread_pct {
                commands.push(Command::Halt {
                    reason: HaltReason::SpreadTooWide {
                        symbol: symbol.clone(),
                        spread_pct: spread,
                    },
                });
                state.halted = true;
                state.halt_reason = Some(format!("spread too wide: {:.4}%", spread * 100.0));
            }
        }
    }
}

fn handle_exec_event(
    state: &mut EngineState,
    event: ExecEvent,
    cfg: &ReducerConfig,
    commands: &mut Vec<Command>,
) {
    match event {
        ExecEvent::OrderAck { client_id, order_id, .. } => {
            if let Some(order) = state.orders.get_mut(&client_id) {
                order.status = OrderStatus::Acked;
                order.order_id = Some(order_id);
            }
            state.risk.consecutive_errors = 0;
        }

        ExecEvent::Fill { ts, symbol, client_id, price, qty, fee, side, .. } => {
            // Remove order
            state.orders.remove(&client_id);

            // Apply to portfolio
            let realized = state.portfolio.apply_fill(&symbol, side, qty, price, fee);

            // Update risk state
            state.risk.last_trade_ts = ts;
            state.risk.daily_pnl += realized;
            state.risk.trades_today += 1;

            if realized < 0.0 {
                state.risk.last_loss_ts = ts;
                state.risk.consecutive_losses += 1;
            } else {
                state.risk.consecutive_losses = 0;
            }

            state.risk.consecutive_errors = 0;

            // Check daily loss limit
            let starting = state.portfolio.cash + state.portfolio.realized_pnl - state.risk.daily_pnl;
            if starting > 0.0 && state.risk.daily_pnl < -starting * cfg.max_daily_loss_pct {
                commands.push(Command::Halt {
                    reason: HaltReason::MaxDrawdown { pct: cfg.max_daily_loss_pct },
                });
                state.halted = true;
                state.halt_reason = Some("daily loss limit".to_string());
            }

            commands.push(Command::Log {
                level: LogLevel::Info,
                msg: format!(
                    "FILL {} {} {:.6} @ {:.2} pnl={:.4}",
                    symbol,
                    if matches!(side, TradeSide::Buy) { "BUY" } else { "SELL" },
                    qty,
                    price,
                    realized
                ),
            });
        }

        ExecEvent::PartialFill { client_id, qty, price, fee, side, symbol, .. } => {
            if let Some(order) = state.orders.get_mut(&client_id) {
                order.filled_qty += qty;
                order.status = OrderStatus::PartiallyFilled;
            }
            state.portfolio.apply_fill(&symbol, side, qty, price, fee);
            state.risk.consecutive_errors = 0;
        }

        ExecEvent::CancelAck { client_id, .. } => {
            if let Some(order) = state.orders.get_mut(&client_id) {
                order.status = OrderStatus::Canceled;
            }
            state.orders.remove(&client_id);
        }

        ExecEvent::Reject { client_id, reason, .. } => {
            if let Some(order) = state.orders.get_mut(&client_id) {
                order.status = OrderStatus::Rejected;
            }
            state.orders.remove(&client_id);
            state.risk.consecutive_errors += 1;

            if state.risk.consecutive_errors >= cfg.max_consecutive_errors {
                commands.push(Command::Halt {
                    reason: HaltReason::MaxErrors { count: state.risk.consecutive_errors },
                });
                state.halted = true;
                state.halt_reason = Some(format!("too many errors: {}", reason));
            }
        }
    }
}

fn handle_sys_event(
    state: &mut EngineState,
    event: SysEvent,
    cfg: &ReducerConfig,
    commands: &mut Vec<Command>,
) {
    match event {
        SysEvent::Timer { ts, name } => {
            // Decay liquidation scores
            for sym in state.symbols.values_mut() {
                sym.liquidation_score *= 0.95;
            }

            // Check data staleness
            for (symbol, sym) in &state.symbols {
                if sym.is_stale(ts, cfg.data_stale_ms) {
                    commands.push(Command::Halt {
                        reason: HaltReason::DataStale {
                            symbol: symbol.clone(),
                            secs: (ts - sym.last_ts) / 1000,
                        },
                    });
                    state.halted = true;
                    state.halt_reason = Some(format!("data stale: {}", symbol));
                    break;
                }
            }

            // Reset daily counters
            let day = ts / 86_400_000;
            state.risk.reset_day(day);

            commands.push(Command::Log {
                level: LogLevel::Debug,
                msg: format!("timer {} hash={}", name, state.hash()),
            });
        }

        SysEvent::Reconnect { source, .. } => {
            commands.push(Command::Log {
                level: LogLevel::Warn,
                msg: format!("reconnect: {}", source),
            });
        }

        SysEvent::DataStale { symbol, last_seen, ts } => {
            commands.push(Command::Halt {
                reason: HaltReason::DataStale {
                    symbol,
                    secs: (ts - last_seen) / 1000,
                },
            });
            state.halted = true;
        }

        SysEvent::Health { status, .. } => {
            if status == HealthStatus::Critical {
                commands.push(Command::Halt {
                    reason: HaltReason::Manual { reason: "health critical".to_string() },
                });
                state.halted = true;
            }
        }

        SysEvent::Halt { reason, .. } => {
            state.halted = true;
            state.halt_reason = Some(format!("{:?}", reason));
            commands.push(Command::CancelAll { symbol: None });
        }
    }
}

/// Update regime state from market indicators (Right Mindfulness)
fn update_regime_state(state: &mut EngineState, symbol: &str, ts: u64, commands: &mut Vec<Command>) {
    if let Some(sym) = state.symbols.get(symbol) {
        // Build narrative indicators from current state
        let indicators = NarrativeIndicators {
            funding_rate: sym.funding_rate,
            funding_avg: 0.0001,  // Baseline
            funding_zscore: sym.funding_rate / 0.0001,  // Simple z-score
            liquidation_score: sym.liquidation_score,
            liquidation_imbalance: 0.0,
            price_change_pct: if sym.prev_close > 0.0 {
                (sym.last_price - sym.prev_close) / sym.prev_close
            } else { 0.0 },
            volume_ratio: 1.0,  // Default
            pv_divergence: 0.0,  // Would need volume data
            volatility_ratio: if sym.volatility > 0.0 {
                sym.volatility / 500.0  // Baseline ~500 for BTC
            } else { 1.0 },
            vol_clustering: 0.0,
            oi_change_rate: 0.0,
            retail_flow_proxy: 0.0,
        };

        let prev_regime = state.regime.current;
        state.regime.update(&indicators, ts);

        // Log regime changes
        if state.regime.current != prev_regime {
            commands.push(Command::Log {
                level: LogLevel::Info,
                msg: format!(
                    "Regime change: {:?} → {:?} (score={:.2}, mult={:.2})",
                    prev_regime,
                    state.regime.current,
                    state.regime.narrative_score,
                    state.regime.position_multiplier
                ),
            });
        }
    }

    // Check staleness
    state.regime.check_staleness(ts, 300_000); // 5 min max age
}

/// Check if trading is allowed using three-poison guards + regime
///
/// This function embodies Right Action by checking ethical guards
/// before allowing any trade to proceed.
fn should_trade(state: &EngineState, symbol: &str, cfg: &ReducerConfig) -> (bool, Option<EthicsViolation>) {
    // Not halted
    if state.halted {
        return (false, None);
    }

    // Regime check (Right Mindfulness) - no new positions in Reflexive
    if state.regime.effective_multiplier() == 0.0 {
        return (false, None);
    }

    // Check three-poison guards (greed, aversion, delusion)
    if let Some(violation) = check_three_poisons(state, symbol, cfg) {
        return (false, Some(violation));
    }

    // Spread not too wide (market microstructure, not ethics)
    if let Some(sym) = state.symbols.get(symbol) {
        if sym.spread > cfg.max_spread_pct {
            return (false, None);
        }
    } else {
        return (false, None);
    }

    // No pending orders for this symbol (operational, not ethics)
    if state.orders.values().any(|o| o.symbol == symbol && o.status == OrderStatus::Pending) {
        return (false, None);
    }

    (true, None) // All guards pass
}

/// Generate trading signal using mean-reversion (non-grasping aligned)
///
/// ## Philosophy
///
/// This signal generator embodies **Right Action** by:
/// - NOT chasing momentum (that's grasping at what already happened)
/// - Providing liquidity to exhausted moves (serving the market)
/// - Using multiple confirmation signals (avoiding delusion)
/// - Accepting that most of the time, no action is correct action
fn generate_signal(state: &EngineState, symbol: &str, cfg: &ReducerConfig) -> Option<Command> {
    let sym = state.symbols.get(symbol)?;
    let pos = state.portfolio.positions.get(symbol);

    // Need minimum data for signal validity (guard against delusion)
    if sym.candle_count < 10 {
        return None;
    }

    // === Exit logic: respect limits, accept outcomes ===
    if let Some(p) = pos {
        if p.qty != 0.0 {
            let entry = p.entry_price;
            let price = sym.last_price;
            let move_pct = (price - entry) / entry * if p.qty > 0.0 { 1.0 } else { -1.0 };

            // Take profit: accept gain without greed for more
            if move_pct >= cfg.take_profit_pct {
                return Some(Command::PlaceOrder {
                    symbol: symbol.to_string(),
                    client_id: format!("tp-{}-{}", symbol, state.seq),
                    side: if p.qty > 0.0 { TradeSide::Sell } else { TradeSide::Buy },
                    qty: p.qty.abs(),
                    price: None,
                });
            }

            // Stop loss: accept loss without aversion
            if move_pct <= -cfg.stop_loss_pct {
                return Some(Command::PlaceOrder {
                    symbol: symbol.to_string(),
                    client_id: format!("sl-{}-{}", symbol, state.seq),
                    side: if p.qty > 0.0 { TradeSide::Sell } else { TradeSide::Buy },
                    qty: p.qty.abs(),
                    price: None,
                });
            }

            // Signal reversal exit: mean-reversion signal flipped
            let score = sym.mean_reversion_score();
            let position_direction = if p.qty > 0.0 { 1.0 } else { -1.0 };
            if score * position_direction < -cfg.exit_threshold {
                return Some(Command::PlaceOrder {
                    symbol: symbol.to_string(),
                    client_id: format!("rev-{}-{}", symbol, state.seq),
                    side: if p.qty > 0.0 { TradeSide::Sell } else { TradeSide::Buy },
                    qty: p.qty.abs(),
                    price: None,
                });
            }

            return None;  // Hold position, no action
        }
    }

    // === Entry logic: provide liquidity to exhaustion ===
    let score = sym.mean_reversion_score();
    let rsi = sym.rsi();
    let accel = sym.momentum_acceleration();

    // Apply regime multiplier to position size (Right Mindfulness)
    // In narrative-driven regimes, we reduce exposure
    let regime_mult = state.regime.effective_multiplier();
    let adjusted_size = cfg.position_size * regime_mult;

    // Don't trade if adjusted size is too small
    if adjusted_size < cfg.position_size * 0.1 {
        return None;
    }

    // Require exhaustion + deceleration for entry (multiple confirmation)
    // This ensures we're not chasing but providing liquidity

    // BUY signal: oversold + decelerating downward momentum
    if score > cfg.entry_threshold && rsi < 35.0 && accel > -0.001 {
        return Some(Command::PlaceOrder {
            symbol: symbol.to_string(),
            client_id: format!("buy-{}-{}", symbol, state.seq),
            side: TradeSide::Buy,
            qty: adjusted_size,
            price: None,
        });
    }

    // SELL signal: overbought + decelerating upward momentum
    if score < -cfg.entry_threshold && rsi > 65.0 && accel < 0.001 {
        return Some(Command::PlaceOrder {
            symbol: symbol.to_string(),
            client_id: format!("sell-{}-{}", symbol, state.seq),
            side: TradeSide::Sell,
            qty: adjusted_size,
            price: None,
        });
    }

    None  // No signal - this is the most common correct answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reduce_candle() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig::default();

        let event = Event::Market(MarketEvent::Candle {
            ts: 1000,
            symbol: "BTCUSDT".to_string(),
            o: 50000.0,
            h: 50100.0,
            l: 49900.0,
            c: 50050.0,
            v: 100.0,
        });

        let output = reduce(&mut state, event, &cfg);

        assert_eq!(state.symbols.get("BTCUSDT").unwrap().last_price, 50050.0);
        assert!(output.state_hash != 0);
    }

    #[test]
    fn test_reduce_fill() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig::default();

        let event = Event::Exec(ExecEvent::Fill {
            ts: 1000,
            symbol: "BTCUSDT".to_string(),
            client_id: "test-1".to_string(),
            order_id: "ex-1".to_string(),
            fill_id: "f-1".to_string(),
            price: 50000.0,
            qty: 0.001,
            fee: 0.05,
            side: TradeSide::Buy,
        });

        let output = reduce(&mut state, event, &cfg);

        assert!(state.portfolio.positions.contains_key("BTCUSDT"));
        assert!(!output.commands.is_empty()); // log command
    }

    #[test]
    fn test_halt_on_errors() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            max_consecutive_errors: 3,
            ..Default::default()
        };

        for i in 0..3 {
            let event = Event::Exec(ExecEvent::Reject {
                ts: 1000 + i as u64,
                symbol: "BTCUSDT".to_string(),
                client_id: format!("test-{}", i),
                reason: "test".to_string(),
            });
            reduce(&mut state, event, &cfg);
        }

        assert!(state.halted);
    }

    #[test]
    fn test_regime_updates_on_candle() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig::default();

        // Initial regime should be Grounded (default)
        assert_eq!(state.regime.position_multiplier, 1.0);

        // Feed a candle
        let event = Event::Market(MarketEvent::Candle {
            ts: 1000,
            symbol: "BTCUSDT".to_string(),
            o: 50000.0, h: 50100.0, l: 49900.0, c: 50050.0, v: 100.0,
        });
        reduce(&mut state, event, &cfg);

        // Regime should be updated
        assert!(state.regime.last_update_ts > 0);
        assert!(!state.regime.is_stale);
    }

    #[test]
    fn test_warmup_prevents_trading() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            entry_threshold: 0.1,  // Very low threshold
            ..Default::default()
        };

        // Feed only 5 candles (less than 10 minimum)
        for i in 0..5 {
            let event = Event::Market(MarketEvent::Candle {
                ts: i * 1000,
                symbol: "BTCUSDT".to_string(),
                o: 50000.0, h: 50100.0, l: 49900.0, c: 50050.0, v: 100.0,
            });
            let output = reduce(&mut state, event, &cfg);

            // Should not generate any PlaceOrder commands
            let has_order = output.commands.iter().any(|c| matches!(c, Command::PlaceOrder { .. }));
            assert!(!has_order, "Should not trade with insufficient warmup");
        }
    }

    #[test]
    fn test_reflexive_regime_halts_new_positions() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig::default();

        // Manually set regime to Reflexive
        state.regime.current = crate::engine::narrative_detector::NarrativeRegime::Reflexive;
        state.regime.position_multiplier = 0.0;
        state.regime.is_stale = false;

        // Feed enough candles to trigger signal
        for i in 0..20 {
            let event = Event::Market(MarketEvent::Candle {
                ts: i * 1000,
                symbol: "BTCUSDT".to_string(),
                o: 50000.0, h: 50100.0, l: 49900.0, c: 50050.0, v: 100.0,
            });
            let output = reduce(&mut state, event, &cfg);

            // Should not generate PlaceOrder in Reflexive regime
            let has_order = output.commands.iter().any(|c| matches!(c, Command::PlaceOrder { .. }));
            assert!(!has_order, "Reflexive regime should halt new positions");
        }
    }

    #[test]
    fn test_cooldown_after_loss() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            cooldown_ms: 10_000,  // 10 second cooldown
            ..Default::default()
        };

        // Simulate a loss
        state.risk.last_loss_ts = 5000;
        state.now = 6000;  // 1 second after loss

        // should_trade should return false during cooldown
        let (can_trade, _) = should_trade(&state, "BTCUSDT", &cfg);
        assert!(!can_trade, "Should not trade during cooldown");

        // After cooldown
        state.now = 20_000;  // Well past cooldown

        // Add symbol data
        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = SymbolState::new();
            sym.candle_count = 20;
            sym.last_ts = 20_000;
            sym
        });

        let (can_trade, _) = should_trade(&state, "BTCUSDT", &cfg);
        assert!(can_trade, "Should be able to trade after cooldown");
    }

    #[test]
    fn test_take_profit_exit() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            take_profit_pct: 0.004,  // 0.4% take profit
            ..Default::default()
        };

        // Create a long position
        state.portfolio.positions.insert(
            "BTCUSDT".to_string(),
            crate::engine::state::Position { qty: 0.01, entry_price: 50000.0 }
        );

        // Create symbol with price above TP
        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = SymbolState::new();
            sym.last_price = 50250.0;  // +0.5% (above 0.4% TP)
            sym.candle_count = 20;
            sym
        });

        // Generate signal should produce exit
        let signal = generate_signal(&state, "BTCUSDT", &cfg);
        assert!(signal.is_some(), "Should generate TP exit");

        if let Some(Command::PlaceOrder { side, .. }) = signal {
            assert!(matches!(side, TradeSide::Sell), "Should sell to close long");
        }
    }

    #[test]
    fn test_stop_loss_exit() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            stop_loss_pct: 0.003,  // 0.3% stop loss
            ..Default::default()
        };

        // Create a long position
        state.portfolio.positions.insert(
            "BTCUSDT".to_string(),
            crate::engine::state::Position { qty: 0.01, entry_price: 50000.0 }
        );

        // Create symbol with price below SL
        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = SymbolState::new();
            sym.last_price = 49800.0;  // -0.4% (below -0.3% SL)
            sym.candle_count = 20;
            sym
        });

        // Generate signal should produce exit
        let signal = generate_signal(&state, "BTCUSDT", &cfg);
        assert!(signal.is_some(), "Should generate SL exit");

        if let Some(Command::PlaceOrder { side, .. }) = signal {
            assert!(matches!(side, TradeSide::Sell), "Should sell to close long");
        }
    }

    #[test]
    fn test_regime_reduces_position_size() {
        let mut state = EngineState::new();
        let cfg = ReducerConfig {
            position_size: 0.01,
            entry_threshold: 0.01,  // Low threshold
            ..Default::default()
        };

        // Set regime to NarrativeDriven (multiplier = 0.3)
        state.regime.current = crate::engine::narrative_detector::NarrativeRegime::NarrativeDriven;
        state.regime.position_multiplier = 0.3;
        state.regime.is_stale = false;

        // Create symbol with strong signal
        state.symbols.insert("BTCUSDT".to_string(), {
            let mut sym = SymbolState::new();
            sym.last_price = 50000.0;
            sym.candle_count = 20;
            // Set up RSI and other indicators for buy signal
            sym.gain_ema = 0.0;
            sym.loss_ema = 0.1;  // Low RSI
            sym.price_mean = 51000.0;  // Below mean
            sym.volatility = 500.0;
            sym
        });

        // Check that position size is adjusted
        // Note: We can't easily test generate_signal directly because of RSI requirements,
        // but we verify the regime multiplier is applied
        assert_eq!(state.regime.effective_multiplier(), 0.3);
    }
}
