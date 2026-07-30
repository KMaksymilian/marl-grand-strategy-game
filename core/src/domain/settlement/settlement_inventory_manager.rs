use std::collections::HashMap;
use crate::domain::economy::resource::{ResourceInventory, ResourceType};

#[derive(Debug)]
pub struct SettlementInventoryManager {
    pub main: ResourceInventory,
    pub pending_production: ResourceInventory,
}

impl SettlementInventoryManager {
    pub fn new(capacity: u32) -> Self {
        Self {
            main: ResourceInventory::new(capacity),
            pending_production: ResourceInventory::new(capacity),
        }
    }

    /// Resetuje i ustawia nową pojemność siły roboczej na daną turę
    pub fn refresh_labor_capacity(&mut self, population: u32) {
        self.main.resources.insert(ResourceType::Labour, population);
    }

    /// Odkłada wyprodukowane rzeczy do bufora, a zwrócone surowce oddaje od razu
    pub fn process_production_results(
        &mut self,
        produced: HashMap<ResourceType, u32>,
        returned: HashMap<ResourceType, u32>,
    ) {
        for (res, amount) in returned {
            self.main.add(res, amount);
        }
        for (res, amount) in produced {
            self.pending_production.add(res, amount);
        }
    }

    /// Finalizuje turę, przenosząc zawartość bufora do głównego magazynu
    pub fn commit_pending_production(&mut self) {
        for (res, amount) in self.pending_production.resources.drain() {
            self.main.add(res, amount);
        }
    }
}