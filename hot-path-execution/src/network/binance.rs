use async_trait::async_trait;
use crate::engine::executor::OrderExecutor;
use crate::config::Config;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, error};

pub struct BinanceExecutor {
    pub config: Config,
    pub client: Client,
}

impl BinanceExecutor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.config.binance_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    async fn send_order(&self, pair: &str, side: &str, price: f64, qty: f64) -> anyhow::Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();
        
        // Nota: Em produção real, você usaria o endpoint correto e lidaria com precisão de decimais.
        let query = format!(
            "symbol={}&side={}&type=LIMIT&timeInForce=GTC&quantity={:.5}&price={:.2}&timestamp={}",
            pair, side, qty, price, timestamp
        );
        
        let signature = self.sign(&query);
        let url = format!("https://api.binance.com/api/v3/order?{}&signature={}", query, signature);

        let response = self.client.post(&url)
            .header("X-MBX-APIKEY", &self.config.binance_api_key)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            let body = response.text().await?;
            info!("[BINANCE EXECUTION] Ordem enviada com sucesso: {}", body);
            Ok(())
        } else {
            let err_text = response.text().await?;
            error!("[BINANCE ERROR] Status: {} | Body: {}", status, err_text);
            Err(anyhow::anyhow!("Binance API Error: {}", err_text))
        }
    }
}

#[async_trait]
impl OrderExecutor for BinanceExecutor {
    async fn get_available_balance(&self) -> f64 {
        // Em produção, isso chamaria GET /api/v3/account
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
        info!(">>> EXECUTANDO ORDENS REAIS NA BINANCE <<<");
        
        // Perna 1: Compra Target
        let leg1 = self.send_order(target_pair, "BUY", target_price, target_qty);
        // Perna 2: Venda Hedge
        let leg2 = self.send_order(hedge_pair, "SELL", hedge_price, hedge_qty);

        let (r1, r2) = tokio::join!(leg1, leg2);

        if let Err(e) = r1 {
            error!("[LEG 1 FAILED] {:?}", e);
        }
        if let Err(e) = r2 {
            error!("[LEG 2 FAILED] {:?}", e);
        }
    }
}
