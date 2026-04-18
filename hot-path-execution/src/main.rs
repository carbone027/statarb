mod ipc;
mod engine;
mod network;
mod state;
mod types;
pub mod portfolio;

use std::sync::Arc;
use tokio::signal;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::engine::router::{run_router, RiskMatrix};
use crate::network::websocket::run_market_data_stream;
use crate::portfolio::manager::PortfolioManager;
use crate::state::orderbook::LocalMarketState;
use crate::state::shm_reader::SharedMemoryReader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing initialization
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set default subscriber for tracing");

    info!("Starting StatArb Hot-Path Engine with IPC Memory Reading...");

    // In-memory concurrent state initialization
    let market_state = Arc::new(LocalMarketState::new());
    
    // Virtual Portfolio Manager
    let portfolio = Arc::new(PortfolioManager::new());
    
    // Dynamic Risk Matrix via RwLock
    let dynamic_risks = Arc::new(tokio::sync::RwLock::new(Vec::<RiskMatrix>::new()));

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

    // SHM Background Reader Task
    let shm_risks = Arc::clone(&dynamic_risks);
    let token_shm = shutdown_token.clone();
    let shm_task = tokio::spawn(async move {
        // Wait briefly to allow Python to scaffold the SHM buffer initially if started at exact same time
        sleep(Duration::from_millis(500)).await;

        let reader = match SharedMemoryReader::new("statarb_signals") {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to mount SHM: {:?}", e);
                return;
            }
        };

        loop {
            tokio::select! {
                _ = sleep(Duration::from_millis(50)) => {
                    // Poll RAM lock-free
                    if let Some(mut updated_signals) = reader.read_signals() {
                        if !updated_signals.is_empty() {
                            let mut write_guard = shm_risks.write().await;
                            *write_guard = std::mem::take(&mut updated_signals);
                        }
                    }
                }
                _ = token_shm.cancelled() => {
                    info!("SHM memory polling terminated gracefully.");
                    break;
                }
            }
        }
    });

    // Pass mathematical engine to tokio::spawn
    let state_router = Arc::clone(&market_state);
    let token_router = shutdown_token.clone();
    let router_risks = Arc::clone(&dynamic_risks);
    let router_portfolio = Arc::clone(&portfolio);
    let router_task = tokio::spawn(async move {
        run_router(state_router, router_risks, router_portfolio, token_router).await;
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
    let _ = tokio::join!(network_task, shm_task, router_task);

    info!("All active handles exited gracefully. Goodbye!");

    Ok(())
}
