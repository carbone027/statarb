use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

use crate::types::models::BookTicker;

pub struct LocalMarketState {
    pub is_valid: AtomicBool,
    pub orderbook: DashMap<String, BookTicker>,
    pub price_notifier: Notify,
}

impl LocalMarketState {
    pub fn new() -> Self {
        Self {
            is_valid: AtomicBool::new(false),
            orderbook: DashMap::new(),
            price_notifier: Notify::new(),
        }
    }

    /// Checks if the state is currently valid and considered fresh without locks
    pub fn valid(&self) -> bool {
        self.is_valid.load(Ordering::Acquire)
    }

    /// Sets the validity of the circuit breaker atomically
    pub fn set_valid(&self, valid: bool) {
        self.is_valid.store(valid, Ordering::Release);
    }

    /// Invalidate the orderbook on websocket disconnects
    pub fn invalidate(&self) {
        self.set_valid(false);
        self.orderbook.clear();
    }
}
