mod network;
mod types;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::network::websocket::run_market_data_stream;
use crate::types::models::LocalMarketState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing initialization
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set default subscriber for tracing");

    info!("Starting StatArb Hot-Path Engine...");

    // In-memory state initialization
    let market_state = Arc::new(RwLock::new(LocalMarketState::new()));

    // Target pairs
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];

    // Pass reactive control to tokio::spawn
    let state_clone = Arc::clone(&market_state);
    let _network_task = tokio::spawn(async move {
        run_market_data_stream(state_clone, symbols).await;
    });

    // Simulated Reactive Router on the main thread
    // In production, this would be an event-driven task or channel receiver,
    // but here we simulate a tight loop checking the state
    loop {
        {
            let state = market_state.read().await;

            // Router must check if state is valid (Circuit Breaker)
            if state.is_valid {
                // Engine is free to read the book and take decisions
                if let (Some(btc), Some(eth)) = (
                    state.orderbook.get("BTCUSDT"),
                    state.orderbook.get("ETHUSDT"),
                ) {
                    // Log the simulated spread
                    // In a real scenario, this would be a high-performance cross-check calculation
                    info!(
                        "Arbitrage Check | BTC Bid: {:.2} / ETH Ask: {:.2} | Valid Spread State",
                        btc.bid_price, eth.ask_price
                    );
                }
            } else {
                tracing::warn!("Market state invalid via Circuit Breaker. Pausing simulations...");
            }
        }
        // Small sleep to avoid hammering the CPU in this simulation
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    }

    #[allow(unreachable_code)]
    {
        Ok(())
    }
}
