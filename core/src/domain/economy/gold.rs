#[derive(Debug)]
pub struct GoldInventoryManager {
    pub gold_amount: u32,
}

impl GoldInventoryManager {
    pub fn new() -> GoldInventoryManager {
        GoldInventoryManager { gold_amount: 0 }
    }
    pub fn total_amount(&self) -> u32 {
        self.gold_amount
    }
    pub fn add(&mut self, amount: u32) {
        self.gold_amount += amount;
    }
    pub fn consume(&mut self, amount: u32) -> bool {
        if self.gold_amount < amount {
            return false;
        }
        self.gold_amount -= amount;
        true
    }
    pub fn take_all(&mut self) -> u32 {
        std::mem::take(&mut self.gold_amount)
    }
}