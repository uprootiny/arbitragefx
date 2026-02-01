//! Core event-driven engine with deterministic replay semantics.
//!
//! Architecture:
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  Data Feeds  │────►│  Event Bus   │────►│   Reducer    │
//! │  (REST/WS)   │     │  (ordered)   │     │  (pure fn)   │
//! └──────────────┘     └──────────────┘     └──────────────┘
//!                                                  │
//!                                                  ▼
//!                      ┌──────────────┐     ┌──────────────┐
//!                      │   Commands   │◄────│    State     │
//!                      │  (place/etc) │     │  (hashed)    │
//!                      └──────────────┘     └──────────────┘
//! ```
//!
//! ## Ethical Framework
//!
//! The engine incorporates guards against the three poisons (kleshas):
//! - **Greed**: Position limits, trade limits, fixed sizing
//! - **Aversion**: Cooldowns, pre-defined exits, cascade limits
//! - **Delusion**: Data freshness, minimum history, mean-reversion over momentum
//!
//! See the [`ethics`] module for formalized guards.

pub mod events;
pub mod state;
pub mod reducer;
pub mod bus;
pub mod ethics;
pub mod backtest_ethics;
pub mod backtest_traps;
pub mod eightfold_path;
pub mod narrative_detector;
pub mod experiment_registry;
pub mod drift_tracker;
pub mod policy;
