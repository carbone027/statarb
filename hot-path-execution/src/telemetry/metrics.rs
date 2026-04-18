// =============================================================================
// telemetry/metrics.rs — Métricas Prometheus (Zero-Overhead no Hot-Path)
// =============================================================================
//
// Todas as métricas são registradas globalmente via `lazy_static!` e utilizam
// operações atômicas internamente (sem Mutex, sem RwLock adicionais).
// O hot-path apenas chama `.set()`, `.inc()` ou `.observe()` — operações
// lock-free que se traduzem em stores/loads atômicos.
//
// A serialização para texto Prometheus acontece *exclusivamente* na task HTTP
// (fora do caminho crítico), sob demanda de cada scrape do Grafana/Prometheus.
// =============================================================================

use lazy_static::lazy_static;
use prometheus::{
    Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, Opts, Registry, TextEncoder,
};

lazy_static! {
    /// Registry dedicado — evita colisão com métricas default do processo.
    pub static ref REGISTRY: Registry = Registry::new();

    // -------------------------------------------------------------------------
    // Métricas Financeiras
    // -------------------------------------------------------------------------

    /// Saldo / PnL consolidado do portfólio em tempo real (USDT).
    pub static ref PORTFOLIO_BALANCE: Gauge = {
        let gauge = Gauge::with_opts(
            Opts::new("portfolio_balance_usdt", "Saldo consolidado do portfólio em USDT")
        ).expect("falha ao criar métrica PORTFOLIO_BALANCE");
        REGISTRY.register(Box::new(gauge.clone())).expect("falha ao registrar PORTFOLIO_BALANCE");
        gauge
    };

    /// Tamanho da posição aberta por símbolo (ex: BTCUSDT → 0.05).
    pub static ref ACTIVE_POSITIONS: GaugeVec = {
        let gauge_vec = GaugeVec::new(
            Opts::new("active_positions", "Tamanho da posição aberta por símbolo"),
            &["symbol"],
        ).expect("falha ao criar métrica ACTIVE_POSITIONS");
        REGISTRY.register(Box::new(gauge_vec.clone())).expect("falha ao registrar ACTIVE_POSITIONS");
        gauge_vec
    };

    // -------------------------------------------------------------------------
    // Métricas de Performance
    // -------------------------------------------------------------------------

    /// Latência do ciclo reativo do Router (leitura → decisão → execução) em ms.
    /// Buckets calibrados para HFT: 0.1ms até 100ms.
    pub static ref EXECUTION_LATENCY: Histogram = {
        let histogram = Histogram::with_opts(
            HistogramOpts::new(
                "execution_latency_ms",
                "Latência do ciclo reativo do motor de execução em milissegundos",
            )
            .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0]),
        ).expect("falha ao criar métrica EXECUTION_LATENCY");
        REGISTRY.register(Box::new(histogram.clone())).expect("falha ao registrar EXECUTION_LATENCY");
        histogram
    };
}

/// Serializa todas as métricas registradas no formato texto do Prometheus.
///
/// Chamada exclusivamente pelo handler HTTP — **nunca** no hot-path.
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = String::new();

    // `encode` escreve em um `Write`; usamos Vec<u8> e convertemos.
    let mut byte_buf = Vec::new();
    encoder
        .encode(&metric_families, &mut byte_buf)
        .expect("falha ao codificar métricas Prometheus");
    buffer.push_str(&String::from_utf8(byte_buf).expect("métricas Prometheus não são UTF-8 válido"));

    buffer
}
