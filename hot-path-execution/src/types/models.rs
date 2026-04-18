use serde::{Deserialize, Deserializer};

// Type Aliases for centralized and painless refactoring in the future.
pub type Price = f64;
pub type Vol = f64;

fn parse_f64_from_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize, Clone)]
pub struct BookTicker {
    #[serde(rename = "s")]
    pub symbol: String, // Kept as String to use easily as HashMap key
    #[serde(rename = "b", deserialize_with = "parse_f64_from_str")]
    pub bid_price: Price,
    #[serde(rename = "B", deserialize_with = "parse_f64_from_str")]
    pub bid_qty: Vol,
    #[serde(rename = "a", deserialize_with = "parse_f64_from_str")]
    pub ask_price: Price,
    #[serde(rename = "A", deserialize_with = "parse_f64_from_str")]
    pub ask_qty: Vol,
}
