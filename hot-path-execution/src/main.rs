mod network;
mod state;
mod types;

use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::network::websocket::run_market_data_stream;
use crate::state::orderbook::LocalMarketState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing initialization
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set default subscriber for tracing");

    info!("Starting StatArb Hot-Path Engine...");

    // In-memory concurrent state initialization
    let market_state = Arc::new(LocalMarketState::new());

    // Target pairs
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];

    // Pass reactive control to tokio::spawn
    let state_clone = Arc::clone(&market_state);
    tokio::spawn(async move {
        run_market_data_stream(state_clone, symbols).await;
    });

    // Purely Reactive Router on the main thread
    // Driven entirely by price notifications without spin-locks or sleep polls
    loop {
        // Await the push notification from the WebSocket reader
        market_state.price_notifier.notified().await;

        // Router must check if state is valid (Circuit Breaker) concurrently
        if market_state.valid() {
            // Engine is free to read the book and take decisions using fine-grained locks
            let btc_ref = market_state.orderbook.get("BTCUSDT");
            let eth_ref = market_state.orderbook.get("ETHUSDT");

            if let (Some(btc), Some(eth)) = (btc_ref, eth_ref) {
                // Log the simulated spread
                info!(
                    "Arbitrage React! | BTC Bid: {:.2} / ETH Ask: {:.2} | Valid Spread State",
                    btc.bid_price, eth.ask_price
                );
            }
        } else {
            tracing::warn!("Market state invalid via Circuit Breaker. Re-arming...");
        }
    }
}
