/// GOLD INVENTORY
#[derive(Debug)]
pub struct GoldInventory {
    amount: u64,
}

impl GoldInventory {
    pub fn new(amount: u64) -> Self {
        GoldInventory { amount }
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn add(&mut self, amount: u64) {
        self.amount += amount;
    }

    pub fn consume(&mut self, amount: u64) -> bool {
        if self.amount < amount {
            return false;
        }
        self.amount -= amount;
        true
    }

    pub fn consume_available(&mut self) -> u64 {
        std::mem::take(&mut self.amount)
    }
}

impl Default for GoldInventory {
    fn default() -> Self {
        let amount: u64 = 0;
        GoldInventory::new(amount)
    }
}
