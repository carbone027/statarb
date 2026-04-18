use tracing::info;

pub fn execute_arbitrage_trade(
    target_pair: &str,
    target_price: f64,
    target_qty: f64,
    hedge_pair: &str,
    hedge_price: f64,
    hedge_qty: f64,
    latency_ms: u128,
) {
    // Log estruturado puro da simulação de execução (Sprint Corrente)
    // Aqui no futuro a integração com a corretora injetará Requests reais via reqwest.
    info!(
        "[EXECUTION] BUY {} {} @ {:.2} | SELL {} {} @ {:.2} - Latência estimada: {} ms",
        target_qty, target_pair, target_price, hedge_qty, hedge_pair, hedge_price, latency_ms
    );
}
