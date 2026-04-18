use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

use crate::state::orderbook::LocalMarketState;
use crate::types::models::BookTicker;

const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

pub async fn run_market_data_stream(state: Arc<LocalMarketState>, symbols: Vec<String>) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(32);

    loop {
        info!("Connecting to Binance WebSocket...");

        match connect_and_listen(&state, &symbols).await {
            Ok(_) => {
                warn!("WebSocket disconnected gracefully. Reconnecting...");
                backoff = Duration::from_secs(1); // reset backoff
            }
            Err(e) => {
                error!("WebSocket error: {:?}. Triggering Circuit Breaker...", e);

                // Circuit Breaker: invalidate state and clear orderbook (Lock-Free)
                state.invalidate();

                info!("Waiting {:?} before reconnecting...", backoff);
                sleep(backoff).await;

                // Exponential backoff
                backoff = std::cmp::min(backoff * 2, max_backoff);
            }
        }
    }
}

async fn connect_and_listen(state: &Arc<LocalMarketState>, symbols: &[String]) -> Result<()> {
    let (ws_stream, _) = connect_async(BINANCE_WS_URL)
        .await
        .context("Failed to connect to Binance WebSocket")?;

    let (mut write, mut read) = ws_stream.split();

    // Prepare subscribe message
    let streams: Vec<String> = symbols
        .iter()
        .map(|s| format!("{}@bookTicker", s.to_lowercase()))
        .collect();

    let subscribe_msg = json!({
        "method": "SUBSCRIBE",
        "params": streams,
        "id": 1
    });

    let msg_str = serde_json::to_string(&subscribe_msg)?;
    write
        .send(Message::Text(msg_str.into()))
        .await
        .context("Failed to send subscribe message")?;

    info!("Subscribed to streams: {:?}", streams);

    // Turn off circuit breaker (i.e. make state valid again) lock-free
    state.set_valid(true);
    info!("Circuit Breaker check passed. Engine ready to ingest prices.");

    while let Some(msg) = read.next().await {
        let msg = msg.context("Error reading message from stream")?;

        match msg {
            Message::Text(text) => {
                // Ignore empty or confirmation responses
                if text.contains("\"result\":null") {
                    continue;
                }

                match serde_json::from_str::<BookTicker>(&text) {
                    Ok(ticker) => {
                        // Extra check to only update orderbook if valid
                        if state.valid() {
                            state.orderbook.insert(ticker.symbol.clone(), ticker);
                            // Notifier avoids coalescer sleep loops in the router thread
                            state.price_notifier.notify_waiters();
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse JSON: {}. Message: {}", e, text);
                    }
                }
            }
            Message::Ping(ping) => {
                write.send(Message::Pong(ping)).await?;
            }
            Message::Close(_) => {
                break;
            }
            _ => Default::default(),
        }
    }

    Ok(())
}
