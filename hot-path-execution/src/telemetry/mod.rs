// =============================================================================
// telemetry/mod.rs — Servidor HTTP de Telemetria (axum)
// =============================================================================
//
// Servidor ultraleve que roda em sua própria task Tokio, completamente
// desacoplado do hot-path. Serve exclusivamente o endpoint GET /metrics
// para scrape do Prometheus / Grafana.
// =============================================================================

pub mod metrics;

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tracing::info;

use crate::telemetry::metrics::gather_metrics;

/// Handler HTTP para `GET /metrics`.
///
/// Retorna as métricas no formato texto padrão do Prometheus
/// com o Content-Type correto para scrape automático.
async fn metrics_handler() -> ([(axum::http::header::HeaderName, &'static str); 1], String) {
    let body = gather_metrics();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Inicializa o servidor de telemetria HTTP na porta especificada.
///
/// Esta função é bloqueante (`.await` no serve) — deve ser chamada
/// dentro de um `tokio::spawn` dedicado para não travar o hot-path.
///
/// O servidor respeita o `CancellationToken` para graceful shutdown.
pub async fn run_telemetry_server(
    bind_addr: &str,
    shutdown_token: tokio_util::sync::CancellationToken,
) {
    let app = Router::new().route("/metrics", get(metrics_handler));

    let listener = TcpListener::bind(bind_addr)
        .await
        .expect(&format!("Falha ao fazer bind no endereço {}", bind_addr));

    info!(
        address = bind_addr,
        "Servidor de telemetria HTTP iniciado — GET /metrics disponível"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("Servidor de telemetria recebeu sinal de shutdown.");
        })
        .await
        .expect("Falha ao executar servidor de telemetria");
}
