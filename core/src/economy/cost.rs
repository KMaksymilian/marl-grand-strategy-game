//! Shared integer resource and elapsed-turn cost value.
use super::ResourceType;
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct ProjectCost {
    pub needed_resources: HashMap<ResourceType, u32>,
    pub turn_cost: u32,
}

impl ProjectCost {
    pub fn normalize(&mut self) {
        self.needed_resources.retain(|_, amount| *amount > 0);
    }

    pub fn is_fully_paid(&self) -> bool {
        self.needed_resources.values().all(|amount| *amount == 0) && self.turn_cost == 0
    }
}
