/// GOLD INVENTORY
#[derive(Debug)]
pub struct GoldInventory {
    amount: u32,
}

impl GoldInventory {
    pub fn new(amount: u32) -> Self {
        GoldInventory { amount }
    }

    pub fn add(&mut self, amount: u32) {
        self.amount += amount;
    }

    pub fn consume(&mut self, amount: u32) -> bool {
        if self.amount < amount {
            return false;
        }
        self.amount -= amount;
        true
    }

    pub fn consume_available(&mut self) -> u32 {
        std::mem::take(&mut self.amount)
    }
}

impl Default for GoldInventory {
    fn default() -> Self {
        let amount: u32 = 0;
        GoldInventory::new(amount)
    }
}
