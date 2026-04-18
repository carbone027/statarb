use async_trait::async_trait;
use crate::engine::executor::OrderExecutor;
use crate::config::Config;
use crate::exchange::binance_client::BinanceClient;
use tracing::{info, error};

/// Executor de produção que delega ordens reais ao `BinanceClient`.
///
/// Responsabilidade: orquestrar as pernas da arbitragem (BUY target + SELL hedge).
/// A comunicação HTTP e assinatura criptográfica ficam encapsuladas no `BinanceClient`.
pub struct BinanceExecutor {
    client: BinanceClient,
}

impl BinanceExecutor {
    pub fn new(config: Config) -> Self {
        let client = BinanceClient::new(
            config.binance_api_key,
            config.binance_secret,
        );
        Self { client }
    }
}

#[async_trait]
impl OrderExecutor for BinanceExecutor {
    async fn get_available_balance(&self) -> f64 {
        // TODO: Implementar GET /api/v3/account para saldo real
        1000.0
    }

    async fn execute_arbitrage_trade(
        &self,
        target_pair: &str,
        target_price: f64,
        target_qty: f64,
        hedge_pair: &str,
        hedge_price: f64,
        hedge_qty: f64,
        _latency_ms: u128,
    ) {
        info!(
            ">>> EXECUTANDO ORDENS REAIS NA BINANCE <<< | Target: {} {:.8} @ {:.2} | Hedge: {} {:.8} @ {:.2}",
            target_pair, target_qty, target_price, hedge_pair, hedge_qty, hedge_price
        );
        
        // Perna 1: Compra Target (MARKET)
        let leg1 = self.client.place_market_order(target_pair, "BUY", target_qty);
        // Perna 2: Venda Hedge (MARKET)
        let leg2 = self.client.place_market_order(hedge_pair, "SELL", hedge_qty);

        // Disparo concorrente das duas pernas
        let (r1, r2) = tokio::join!(leg1, leg2);

        if let Err(e) = r1 {
            error!("[LEG 1 FAILED] {} BUY {:.8}: {:?}", target_pair, target_qty, e);
        }
        if let Err(e) = r2 {
            error!("[LEG 2 FAILED] {} SELL {:.8}: {:?}", hedge_pair, hedge_qty, e);
        }
    }
}
