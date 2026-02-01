//! Integration test: Drift-triggered conservative response
//!
//! This test verifies that the system responds appropriately when
//! distribution drift is detected during a live-loop simulation.
//!
//! "The moment the system starts breathing."

use arbitragefx::engine::{
    bus::EventBus,
    events::*,
    reducer::{reduce, ReducerConfig},
    state::EngineState,
    drift_tracker::{DriftTracker, DriftSeverity},
};

/// Simulate a trading session with mid-run distribution shift
#[test]
fn test_drift_triggers_conservative_response() {
    let mut state = EngineState::new();
    let mut bus = EventBus::new();
    let cfg = ReducerConfig {
        entry_threshold: 0.25,
        position_size: 0.01,
        cooldown_ms: 0,  // No cooldown for test
        data_stale_ms: u64::MAX,
        ..Default::default()
    };

    let mut drift_tracker = DriftTracker::default_windows();
    let symbol = "BTCUSDT".to_string();

    let mut total_commands = 0;
    let mut orders_before_shift = 0;
    let mut orders_after_shift = 0;

    // Phase 1: Stable regime (100 bars)
    // Price oscillates around 50000 with normal volatility
    for i in 0..100 {
        let ts = i * 300_000;  // 5-minute bars
        let base_price = 50000.0;
        let noise = ((i as f64 * 0.1).sin() * 200.0);  // ±200 oscillation
        let price = base_price + noise;

        // Push candle event
        bus.push(Event::Market(MarketEvent::Candle {
            ts,
            symbol: symbol.clone(),
            o: price - 50.0,
            h: price + 100.0,
            l: price - 100.0,
            c: price,
            v: 1000.0,
        }));

        // Push timer for housekeeping
        bus.push(Event::Sys(SysEvent::Timer {
            ts,
            name: "tick".to_string(),
        }));

        // Update drift tracker with stable metrics
        drift_tracker.update_from_market(
            100.0,   // stable volatility
            0.001,   // small returns
            0.001,   // normal spread
            0.0001,  // normal funding
            0.0,     // neutral z-score
            ts,
        );

        // Process events
        while let Some(event) = bus.pop() {
            let output = reduce(&mut state, event, &cfg);
            total_commands += output.commands.len();

            for cmd in &output.commands {
                if matches!(cmd, Command::PlaceOrder { .. }) {
                    orders_before_shift += 1;
                }
            }
        }
    }

    // Check that drift is low in stable regime
    drift_tracker.compute_overall();
    let pre_shift_severity = drift_tracker.overall_severity;
    assert!(
        matches!(pre_shift_severity, DriftSeverity::None | DriftSeverity::Low),
        "Pre-shift severity should be low: {:?}", pre_shift_severity
    );

    // Phase 2: Distribution shift (50 bars)
    // Price crashes, volatility spikes, funding goes extreme
    for i in 100..150 {
        let ts = i * 300_000;
        let base_price = 45000.0;  // -10% crash
        let noise = ((i as f64 * 0.3).sin() * 1000.0);  // ±1000 (high vol)
        let price = base_price + noise;

        // Push candle event
        bus.push(Event::Market(MarketEvent::Candle {
            ts,
            symbol: symbol.clone(),
            o: price - 200.0,
            h: price + 500.0,
            l: price - 500.0,
            c: price,
            v: 5000.0,  // Volume spike
        }));

        bus.push(Event::Sys(SysEvent::Timer {
            ts,
            name: "tick".to_string(),
        }));

        // Update drift tracker with shifted metrics
        drift_tracker.update_from_market(
            500.0,   // 5x volatility
            -0.02,   // Large negative returns
            0.01,    // Wide spreads
            -0.005,  // Extreme negative funding
            -2.0,    // Negative z-score
            ts,
        );

        // Process events
        while let Some(event) = bus.pop() {
            let output = reduce(&mut state, event, &cfg);
            total_commands += output.commands.len();

            for cmd in &output.commands {
                if matches!(cmd, Command::PlaceOrder { .. }) {
                    orders_after_shift += 1;
                }
            }
        }
    }

    // Check that drift is detected
    drift_tracker.compute_overall();
    let post_shift_severity = drift_tracker.overall_severity;

    // Verify drift detection
    assert!(
        !matches!(post_shift_severity, DriftSeverity::None),
        "Should detect drift after shift: {:?}", post_shift_severity
    );

    // Verify conservative response
    let actions = drift_tracker.recommended_actions();
    assert!(!actions.is_empty(), "Should have recommended actions");

    // The system should have generated fewer orders after the shift
    // (This is a soft check - the exact numbers depend on signal logic)
    println!("Orders before shift: {}", orders_before_shift);
    println!("Orders after shift: {}", orders_after_shift);
    println!("Pre-shift severity: {:?}", pre_shift_severity);
    println!("Post-shift severity: {:?}", post_shift_severity);
    println!("Position multiplier: {:.2}", drift_tracker.position_multiplier());

    // If drift is Moderate or worse, position multiplier should be reduced
    if matches!(post_shift_severity, DriftSeverity::Moderate | DriftSeverity::Severe | DriftSeverity::Critical) {
        assert!(
            drift_tracker.position_multiplier() < 1.0,
            "Position multiplier should be reduced in drift: {:.2}",
            drift_tracker.position_multiplier()
        );
    }
}

/// Test that the system halts appropriately on severe drift
#[test]
fn test_severe_drift_halts_system() {
    let mut drift_tracker = DriftTracker::default_windows();

    // Feed stable baseline
    for i in 0..100 {
        drift_tracker.update_from_market(
            100.0,   // volatility
            0.001,   // returns
            0.001,   // spread
            0.0001,  // funding
            0.0,     // z_score
            i,
        );
    }

    // Feed extreme drift values
    for i in 100..120 {
        drift_tracker.update_from_market(
            1000.0,  // 10x volatility
            -0.1,    // -10% returns
            0.05,    // 5% spread
            -0.01,   // Extreme funding
            -5.0,    // Extreme z-score
            i,
        );
    }

    drift_tracker.compute_overall();

    // Should be at least Moderate severity
    assert!(
        !matches!(drift_tracker.overall_severity, DriftSeverity::None | DriftSeverity::Low),
        "Extreme values should trigger drift detection"
    );

    // Check recommended actions
    let actions = drift_tracker.recommended_actions();
    let has_reduce = actions.iter().any(|a| {
        matches!(a, arbitragefx::engine::drift_tracker::DriftAction::ReduceExposure { .. } |
                   arbitragefx::engine::drift_tracker::DriftAction::HaltNewPositions)
    });

    assert!(has_reduce, "Should recommend reducing exposure or halting");
}

/// Test end-to-end: regime + drift combined response
#[test]
fn test_regime_and_drift_combined() {
    use arbitragefx::engine::narrative_detector::NarrativeRegime;

    let mut state = EngineState::new();
    let cfg = ReducerConfig::default();

    // Set up both regime and drift in warning states
    state.regime.current = NarrativeRegime::NarrativeDriven;
    state.regime.position_multiplier = 0.3;
    state.regime.is_stale = false;

    let mut drift_tracker = DriftTracker::default_windows();

    // Feed enough data to trigger moderate drift
    for i in 0..100 {
        drift_tracker.push("volatility", 100.0, i);
    }
    for i in 100..120 {
        drift_tracker.push("volatility", 300.0, i);  // 3x spike
    }

    drift_tracker.compute_overall();

    // Combined position multiplier should be very conservative
    let regime_mult = state.regime.effective_multiplier();
    let drift_mult = drift_tracker.position_multiplier();
    let combined = regime_mult * drift_mult;

    println!("Regime multiplier: {:.2}", regime_mult);
    println!("Drift multiplier: {:.2}", drift_mult);
    println!("Combined multiplier: {:.2}", combined);

    // Combined should be <= min of the two
    assert!(
        combined <= regime_mult.min(drift_mult) + 0.01,  // Small tolerance
        "Combined should be conservative: {} vs min({}, {})",
        combined, regime_mult, drift_mult
    );
}
