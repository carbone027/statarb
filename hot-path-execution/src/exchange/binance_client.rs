use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, error};

/// Resposta de erro da Binance REST API.
/// Ref: https://binance-docs.github.io/apidocs/spot/en/#error-codes
#[derive(serde::Deserialize, Debug)]
struct BinanceApiError {
    code: i64,
    msg: String,
}

/// Comunicador de baixo nível com a Binance REST API.
///
/// Responsabilidade única: montar requisições HTTP assinadas e dispará-las.
/// Não contém lógica de negócio, sizing ou portfolio — isso pertence ao `Executor`.
pub struct BinanceClient {
    api_key: String,
    secret_key: String,
    client: Client,
    base_url: String,
}

impl BinanceClient {
    /// Cria uma nova instância reutilizável do client.
    /// O `reqwest::Client` interno mantém um pool de conexões HTTP/2.
    pub fn new(api_key: String, secret_key: String) -> Self {
        Self {
            api_key,
            secret_key,
            client: Client::new(),
            base_url: "https://api.binance.com".to_string(),
        }
    }

    /// Gera a assinatura HMAC-SHA256 de uma query string conforme a
    /// documentação oficial da Binance:
    /// https://binance-docs.github.io/apidocs/spot/en/#signed-trade-and-user_data-endpoint-security
    fn sign_payload(&self, query_string: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret_key.as_bytes())
            .expect("HMAC-SHA256 aceita chaves de qualquer tamanho");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Envia uma ordem a mercado (MARKET) para a Binance.
    ///
    /// # Argumentos
    /// * `symbol` - Par de trading (ex: "BTCUSDT")
    /// * `side`   - Lado da operação ("BUY" ou "SELL")
    /// * `quantity` - Quantidade do ativo base
    ///
    /// # Erros
    /// Retorna `anyhow::Error` em caso de falha de rede ou rejeição da API
    /// com código e mensagem explícitos da Binance.
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
    ) -> Result<()> {
        // 1. Timestamp em milissegundos (exigido pela API signed)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Falha ao obter timestamp do sistema")?
            .as_millis();

        // 2. Montar a query string canônica
        let query_string = format!(
            "symbol={symbol}&side={side}&type=MARKET&quantity={quantity:.8}&timestamp={timestamp}"
        );

        // 3. Assinar com HMAC-SHA256
        let signature = self.sign_payload(&query_string);

        // 4. Disparar POST para o endpoint de ordens
        let url = format!(
            "{}/api/v3/order?{}&signature={}",
            self.base_url, query_string, signature
        );

        info!(
            "[BINANCE CLIENT] Enviando ordem: {} {} {:.8} {}",
            side, symbol, quantity, url
        );

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .context("Falha na conexão HTTP com a Binance")?;

        // 5. Tratar resposta
        let status = response.status();
        let body = response
            .text()
            .await
            .context("Falha ao ler body da resposta da Binance")?;

        if status.is_success() {
            info!(
                "[BINANCE CLIENT] Ordem executada com sucesso | Status: {} | Resposta: {}",
                status, body
            );
            Ok(())
        } else {
            // Tentar deserializar o erro estruturado da Binance
            match serde_json::from_str::<BinanceApiError>(&body) {
                Ok(api_err) => {
                    error!(
                        "[BINANCE CLIENT] Ordem REJEITADA | Código: {} | Mensagem: {}",
                        api_err.code, api_err.msg
                    );
                    Err(anyhow::anyhow!(
                        "Binance API Error (code: {}): {}",
                        api_err.code,
                        api_err.msg
                    ))
                }
                Err(_) => {
                    error!(
                        "[BINANCE CLIENT] Erro inesperado | Status: {} | Body: {}",
                        status, body
                    );
                    Err(anyhow::anyhow!(
                        "Binance HTTP Error (status: {}): {}",
                        status,
                        body
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_signature_deterministic() {
        let client = BinanceClient::new(
            "test_api_key".to_string(),
            "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j".to_string(),
        );

        // Vetor de teste baseado na documentação oficial da Binance
        let payload = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let signature = client.sign_payload(payload);

        // A assinatura deve ser determinística para o mesmo payload + secret
        assert_eq!(signature.len(), 64, "HMAC-SHA256 hex deve ter 64 caracteres");
        assert!(
            signature.chars().all(|c| c.is_ascii_hexdigit()),
            "Assinatura deve conter apenas hex digits"
        );
    }

    #[test]
    fn test_sign_payload_known_vector() {
        // Vetor de teste extraído da documentação Binance:
        // https://binance-docs.github.io/apidocs/spot/en/#signed-trade-and-user_data-endpoint-security
        let client = BinanceClient::new(
            "vmPUZE6mv9SD5VNHk4HlWFsOr6aKE2zvsw0MuIgwCIPy6utIco14y7Ju91duEh8A".to_string(),
            "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j".to_string(),
        );

        let payload = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let signature = client.sign_payload(payload);

        // Resultado esperado pela documentação oficial
        assert_eq!(
            signature,
            "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71"
        );
    }
}
