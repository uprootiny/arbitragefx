//! Event bus with ordering guarantees.

use std::collections::BinaryHeap;
use std::cmp::Ordering;

use super::events::{Event, Timestamp};

/// Wrapper for priority queue ordering (earliest first)
struct TimedEvent {
    ts: Timestamp,
    seq: u64,
    event: Event,
}

impl PartialEq for TimedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.ts == other.ts && self.seq == other.seq
    }
}

impl Eq for TimedEvent {}

impl PartialOrd for TimedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (earliest first)
        match other.ts.cmp(&self.ts) {
            Ordering::Equal => other.seq.cmp(&self.seq),
            ord => ord,
        }
    }
}

/// Event bus with deterministic ordering
pub struct EventBus {
    queue: BinaryHeap<TimedEvent>,
    seq_counter: u64,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            seq_counter: 0,
        }
    }

    /// Push an event onto the bus
    pub fn push(&mut self, event: Event) {
        let ts = event.timestamp();
        self.seq_counter += 1;
        self.queue.push(TimedEvent {
            ts,
            seq: self.seq_counter,
            event,
        });
    }

    /// Pop the earliest event
    pub fn pop(&mut self) -> Option<Event> {
        self.queue.pop().map(|te| te.event)
    }

    /// Peek at the earliest event
    pub fn peek(&self) -> Option<&Event> {
        self.queue.peek().map(|te| &te.event)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Number of pending events
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Drain all events in order
    pub fn drain(&mut self) -> Vec<Event> {
        let mut events = Vec::with_capacity(self.queue.len());
        while let Some(te) = self.queue.pop() {
            events.push(te.event);
        }
        events
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::events::*;

    #[test]
    fn test_ordering() {
        let mut bus = EventBus::new();

        // Push events out of order
        bus.push(Event::Market(MarketEvent::Candle {
            ts: 3000,
            symbol: "BTC".to_string(),
            o: 0.0, h: 0.0, l: 0.0, c: 0.0, v: 0.0,
        }));

        bus.push(Event::Market(MarketEvent::Candle {
            ts: 1000,
            symbol: "BTC".to_string(),
            o: 0.0, h: 0.0, l: 0.0, c: 0.0, v: 0.0,
        }));

        bus.push(Event::Market(MarketEvent::Candle {
            ts: 2000,
            symbol: "BTC".to_string(),
            o: 0.0, h: 0.0, l: 0.0, c: 0.0, v: 0.0,
        }));

        // Should come out in timestamp order
        assert_eq!(bus.pop().unwrap().timestamp(), 1000);
        assert_eq!(bus.pop().unwrap().timestamp(), 2000);
        assert_eq!(bus.pop().unwrap().timestamp(), 3000);
    }

    #[test]
    fn test_same_timestamp_fifo() {
        let mut bus = EventBus::new();

        // Events with same timestamp should preserve insertion order
        bus.push(Event::Sys(SysEvent::Timer { ts: 1000, name: "first".to_string() }));
        bus.push(Event::Sys(SysEvent::Timer { ts: 1000, name: "second".to_string() }));
        bus.push(Event::Sys(SysEvent::Timer { ts: 1000, name: "third".to_string() }));

        let e1 = bus.pop().unwrap();
        let e2 = bus.pop().unwrap();
        let e3 = bus.pop().unwrap();

        if let Event::Sys(SysEvent::Timer { name, .. }) = e1 {
            assert_eq!(name, "first");
        }
        if let Event::Sys(SysEvent::Timer { name, .. }) = e2 {
            assert_eq!(name, "second");
        }
        if let Event::Sys(SysEvent::Timer { name, .. }) = e3 {
            assert_eq!(name, "third");
        }
    }
}
