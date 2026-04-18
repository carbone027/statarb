use std::sync::Arc;
use tracing::{info, warn};
use crate::portfolio::manager::PortfolioManager;

pub fn execute_arbitrage_trade(
    target_pair: &str,
    target_price: f64,
    target_qty: f64,
    hedge_pair: &str,
    hedge_price: f64,
    hedge_qty: f64,
    latency_ms: u128,
    portfolio: Arc<PortfolioManager>,
) {
    // Calcula o custo das duas operações para reter a margem
    let total_cost = (target_qty * target_price) + (hedge_qty * hedge_price);

    // 2. Fluxo da Ordem: Debitar o custo (simulando a retenção de margem na corretora)
    if let Err(e) = portfolio.allocate_funds(total_cost) {
        warn!("[PAPER TRADE REJECTED] Erro ao alocar margem para o trade: {}", e);
        return;
    }

    // 2. Fluxo da Ordem: Atualizar Posições Long e Short
    portfolio.update_position(target_pair.to_string(), target_qty, target_price);
    portfolio.update_position(hedge_pair.to_string(), -hedge_qty, hedge_price);

    // 3. Log de Estado: Verifica saldo restrito
    let free_balance = *portfolio.available_balance.read().unwrap();
    
    // Ler os saldos atualizados diretamente do Dashmap Locklessly
    let target_pos_qty = portfolio.positions.get(target_pair).map(|p| p.quantity).unwrap_or(0.0);
    let hedge_pos_qty = portfolio.positions.get(hedge_pair).map(|p| p.quantity).unwrap_or(0.0);

    info!(
        "[PAPER TRADE EXECUTADO] Novo Saldo Livre: {:.2} USDT | Posições: [{}: {:.5}, {}: {:.5}] ({}ms latência)",
        free_balance, target_pair, target_pos_qty, hedge_pair, hedge_pos_qty, latency_ms
    );
}
