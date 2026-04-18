use std::sync::Arc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::engine::executor::execute_arbitrage_trade;
use crate::portfolio::manager::PortfolioManager;
use crate::state::orderbook::LocalMarketState;

#[derive(Clone, Debug)]
pub struct RiskMatrix {
    pub target_symbol: String,
    pub hedge_symbol: String,
    pub hedge_ratio: f64,
    pub threshold: f64,
}

pub async fn run_router(
    state: Arc<LocalMarketState>,
    risk_states: Arc<tokio::sync::RwLock<Vec<RiskMatrix>>>,
    portfolio: Arc<PortfolioManager>,
    shutdown_token: CancellationToken,
) {
    info!("Engine Router started. Listening for runtime signal updates.");

    loop {
        tokio::select! {
            _ = state.price_notifier.notified() => {
                let instant_start = Instant::now();

                // Circuit Breaker
                if !state.valid() {
                    warn!("Router blocked by invalid Circuit Breaker. Waiting...");
                    continue;
                }

                // Copy risk states lock-free to evaluate immediately
                let active_risks = risk_states.read().await.clone();

                for risk in active_risks {
                    let btc_ref = state.orderbook.get(&risk.target_symbol);
                    let eth_ref = state.orderbook.get(&risk.hedge_symbol);

                    if let (Some(btc), Some(eth)) = (btc_ref, eth_ref) {
                        // Parametric Logical Spread
                        let simulated_spread = btc.bid_price - (eth.ask_price * risk.hedge_ratio);

                        if simulated_spread > risk.threshold {
                            // Regras de Capital e Sizing
                            let available_balance = *portfolio.available_balance.read().unwrap();
                            let trade_money = available_balance * 0.05; // 5% do saldo livre
                            
                            let target_money = trade_money / 2.0;
                            let hedge_money = trade_money / 2.0;
                            
                            let target_qty = target_money / btc.ask_price;
                            let hedge_qty = (hedge_money / eth.bid_price) * risk.hedge_ratio;
                            
                            if target_qty < 0.0001 || hedge_qty < 0.0001 {
                                warn!(
                                    "Trade ignorado: Qty muito pequena (Target: {:.5}, Hedge: {:.5}) min 0.0001",
                                    target_qty, hedge_qty
                                );
                                continue;
                            }

                            let latency = instant_start.elapsed().as_millis();
                            // Delegate to Executor
                            execute_arbitrage_trade(
                                &risk.target_symbol,
                                btc.ask_price,
                                target_qty,
                                &risk.hedge_symbol,
                                eth.bid_price,
                                hedge_qty,
                                latency,
                                Arc::clone(&portfolio)
                            );
                        }
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
