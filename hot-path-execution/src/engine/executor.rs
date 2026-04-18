use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn, error};
use crate::portfolio::manager::PortfolioManager;
use crate::exchange::binance_client::BinanceClient;

// ─────────────────────────────────────────────────────────────────────────────
// Trait: Abstração de Execução
// O Router depende APENAS deste contrato. Ele não sabe se o dinheiro é real.
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn get_available_balance(&self) -> f64;

    async fn execute_arbitrage_trade(
        &self,
        target_pair: &str,
        target_price: f64,
        target_qty: f64,
        hedge_pair: &str,
        hedge_price: f64,
        hedge_qty: f64,
        latency_ms: u128,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Implementação: Paper Trading (Simulação em Memória)
// Opera exclusivamente no PortfolioManager virtual, sem I/O de rede.
// ─────────────────────────────────────────────────────────────────────────────

pub struct PaperExecutor {
    pub portfolio: Arc<PortfolioManager>,
}

#[async_trait]
impl OrderExecutor for PaperExecutor {
    async fn get_available_balance(&self) -> f64 {
        *self.portfolio.available_balance.read().unwrap()
    }

    async fn execute_arbitrage_trade(
        &self,
        target_pair: &str,
        target_price: f64,
        target_qty: f64,
        hedge_pair: &str,
        hedge_price: f64,
        hedge_qty: f64,
        latency_ms: u128,
    ) {
        // Calcula o custo das duas operações para reter a margem
        let total_cost = (target_qty * target_price) + (hedge_qty * hedge_price);

        // Fluxo da Ordem: Debitar o custo (simulando a retenção de margem na corretora)
        if let Err(e) = self.portfolio.allocate_funds(total_cost) {
            warn!("[PAPER TRADE REJECTED] Erro ao alocar margem para o trade: {}", e);
            return;
        }

        // Fluxo da Ordem: Atualizar Posições Long e Short
        self.portfolio.update_position(target_pair.to_string(), target_qty, target_price);
        self.portfolio.update_position(hedge_pair.to_string(), -hedge_qty, hedge_price);

        // Log de Estado: Verifica saldo restrito
        let free_balance = *self.portfolio.available_balance.read().unwrap();
        
        // Ler os saldos atualizados diretamente do Dashmap Locklessly
        let target_pos_qty = self.portfolio.positions.get(target_pair).map(|p| p.quantity).unwrap_or(0.0);
        let hedge_pos_qty = self.portfolio.positions.get(hedge_pair).map(|p| p.quantity).unwrap_or(0.0);

        info!(
            "[PAPER TRADE EXECUTADO] Novo Saldo Livre: {:.2} USDT | Posições: [{}: {:.5}, {}: {:.5}] ({}ms latência)",
            free_balance, target_pair, target_pos_qty, hedge_pair, hedge_pos_qty, latency_ms
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Implementação: Live Trading (Ordens Reais via BinanceClient)
// Delega a comunicação HTTP e assinatura criptográfica ao BinanceClient.
// ─────────────────────────────────────────────────────────────────────────────

pub struct LiveExecutor {
    client: BinanceClient,
}

impl LiveExecutor {
    pub fn new(client: BinanceClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl OrderExecutor for LiveExecutor {
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
            ">>> EXECUTANDO ORDENS REAIS NA BINANCE <<< | Target: {} BUY {:.8} @ {:.2} | Hedge: {} SELL {:.8} @ {:.2}",
            target_pair, target_qty, target_price, hedge_pair, hedge_qty, hedge_price
        );

        // Disparo concorrente das duas pernas da arbitragem
        let leg1 = self.client.place_market_order(target_pair, "BUY", target_qty);
        let leg2 = self.client.place_market_order(hedge_pair, "SELL", hedge_qty);

        let (r1, r2) = tokio::join!(leg1, leg2);

        match &r1 {
            Ok(()) => info!("[LEG 1 OK] {} BUY {:.8}", target_pair, target_qty),
            Err(e) => error!("[LEG 1 FAILED] {} BUY {:.8}: {:?}", target_pair, target_qty, e),
        }
        match &r2 {
            Ok(()) => info!("[LEG 2 OK] {} SELL {:.8}", hedge_pair, hedge_qty),
            Err(e) => error!("[LEG 2 FAILED] {} SELL {:.8}: {:?}", hedge_pair, hedge_qty, e),
        }
    }
}
