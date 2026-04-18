use std::sync::Arc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::engine::executor::execute_arbitrage_trade;
use crate::state::orderbook::LocalMarketState;

pub struct RiskMatrix {
    pub target_symbol: String,
    pub hedge_symbol: String,
    pub hedge_ratio: f64,
    pub threshold: f64,
}

pub async fn run_router(
    state: Arc<LocalMarketState>,
    risk: RiskMatrix,
    shutdown_token: CancellationToken,
) {
    info!(
        "Engine Router started for {} / {}",
        risk.target_symbol, risk.hedge_symbol
    );

    let base_quantity = 1.0; // Posição base virtual de 1 inteiro (Ex: 1 BTC)

    loop {
        tokio::select! {
            _ = state.price_notifier.notified() => {
                let instant_start = Instant::now();

                // Extração dos Books garantindo segurança do Circuito
                if !state.valid() {
                    warn!("Router blocked by invalid Circuit Breaker. Waiting...");
                    continue;
                }

                let btc_ref = state.orderbook.get(&risk.target_symbol);
                let eth_ref = state.orderbook.get(&risk.hedge_symbol);

                if let (Some(btc), Some(eth)) = (btc_ref, eth_ref) {
                    // Cálculo Lógico Paramétrico
                    // Valor Teórico: bid do alvo vs (ask do Hedge * ratio)
                    let simulated_spread = btc.bid_price - (eth.ask_price * risk.hedge_ratio);

                    if simulated_spread > risk.threshold {
                        let latency = instant_start.elapsed().as_millis();
                        // Delega ao executor.rs e repassa propriedades
                        execute_arbitrage_trade(
                            &risk.target_symbol,
                            btc.ask_price, // Simula a compra que consumirá ask
                            base_quantity,
                            &risk.hedge_symbol,
                            eth.bid_price, // Simula a venda que alvejará bid
                            base_quantity * risk.hedge_ratio,
                            latency
                        );
                    }
                }
            }
            _ = shutdown_token.cancelled() => {
                info!("Shutdown signal received. Router terminating gracefully...");
                break;
            }
        }
    }
}
