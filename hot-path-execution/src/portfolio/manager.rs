use std::sync::RwLock;
use dashmap::DashMap;
use super::position::Position;

#[derive(Debug)]
pub struct PortfolioManager {
    pub available_balance: RwLock<f64>,
    pub locked_balance: RwLock<f64>,
    pub positions: DashMap<String, Position>,
}

impl PortfolioManager {
    pub fn new() -> Self {
        Self {
            // Conta começa com saldo em caixa estático livre para as operações (10k)
            available_balance: RwLock::new(10000.0),
            locked_balance: RwLock::new(0.0),
            positions: DashMap::new(),
        }
    }

    pub fn allocate_funds(&self, amount: f64) -> Result<(), String> {
        let mut available = self.available_balance.write().unwrap();
        let mut locked = self.locked_balance.write().unwrap();
        
        if *available >= amount {
            *available -= amount;
            *locked += amount;
            Ok(())
        } else {
            Err("Saldo livre insuficiente para alocação".to_string())
        }
    }

    pub fn release_funds(&self, amount: f64) -> Result<(), String> {
        // Obter os locks na mesma ordem evita possíveis cenários de Deadlock (se mais métodos fossem adicionados)
        let mut available = self.available_balance.write().unwrap();
        let mut locked = self.locked_balance.write().unwrap();
        
        if *locked >= amount {
            *locked -= amount;
            *available += amount;
            Ok(())
        } else {
            Err("Saldo bloqueado insuficiente para liberação".to_string())
        }
    }

    pub fn update_position(&self, symbol: String, qty: f64, price: f64) {
        self.positions.entry(symbol.clone())
            .and_modify(|pos| {
                let total_cost_old = pos.quantity * pos.average_entry_price;
                let new_cost = qty * price;
                let new_qty = pos.quantity + qty;
                
                pos.quantity = new_qty;
                if new_qty > 0.0 {
                    pos.average_entry_price = (total_cost_old + new_cost) / new_qty;
                } else if new_qty == 0.0 {
                    pos.average_entry_price = 0.0;
                }
            })
            .or_insert_with(|| Position::new(symbol, qty, price));
    }
}
