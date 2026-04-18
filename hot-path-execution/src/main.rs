mod engine;
mod network;
mod state;
mod types;

use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::engine::router::{run_router, RiskMatrix};
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

    // Token for Graceful Shutdown coordination
    let shutdown_token = CancellationToken::new();

    // Target pairs
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];

    // Pass reactive network reader to tokio::spawn
    let state_network = Arc::clone(&market_state);
    let token_network = shutdown_token.clone();
    let network_task = tokio::spawn(async move {
        run_market_data_stream(state_network, symbols, token_network).await;
    });

    // Dummy Risk Matrix setup
    let risk_matrix = RiskMatrix {
        target_symbol: "BTCUSDT".to_string(),
        hedge_symbol: "ETHUSDT".to_string(),
        hedge_ratio: 15.5, // Emulação provisória de balanceamento matemático.
        threshold: 2.0,    // Baseado na regressão linear teórica (simulada).
    };

    // Pass mathematical engine to tokio::spawn
    let state_router = Arc::clone(&market_state);
    let token_router = shutdown_token.clone();
    let router_task = tokio::spawn(async move {
        run_router(state_router, risk_matrix, token_router).await;
    });

    // O Sistema permanece vivo rodando o background async até dispararmos o SIGINT.
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("SIGINT (Ctrl+C) received. Beginning Graceful Shutdown...");
            // Desencadeia o fechamento das sockets e termina as corotinas
            shutdown_token.cancel();
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    info!("Waiting for execution tasks to clean up IO & Network safely...");
    let _ = tokio::join!(network_task, router_task);

    // As the blocks finish and program terminates here, the tracing system is intrinsically flushed.
    // Opcionalmente, pode ser chamado funções de drop explicito do tracer se houver um arquivo.
    info!("All active handles exited gracefully. Goodbye!");

    Ok(())
}
