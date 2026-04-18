use std::env;
use dotenvy::dotenv;

#[derive(Clone, Debug)]
pub struct Config {
    pub binance_api_key: String,
    pub binance_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenv().ok();

        let binance_api_key = env::var("BINANCE_API_KEY")
            .map_err(|_| "BINANCE_API_KEY is not defined in .env".to_string())?;
        
        let binance_secret = env::var("BINANCE_SECRET")
            .map_err(|_| "BINANCE_SECRET is not defined in .env".to_string())?;

        Ok(Self {
            binance_api_key,
            binance_secret,
        })
    }
}
