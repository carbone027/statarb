#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub average_entry_price: f64,
}

impl Position {
    pub fn new(symbol: String, quantity: f64, average_entry_price: f64) -> Self {
        Self {
            symbol,
            quantity,
            average_entry_price,
        }
    }
}
