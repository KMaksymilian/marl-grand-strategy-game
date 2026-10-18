use crate::domain::economy::resource::{ResourceInventory, ResourceType};
use crate::domain::economy::resource_allocation::{AllocationRecipient, AllocationResult};
pub use crate::domain::settlement::population::ConsumptionResult;
use crate::domain::settlement::projects::ProjectOutcome;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type MaterialTotals = BTreeMap<ResourceType, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryLossStage {
    Returns,
    ProductionBuffer,
    Warehouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryLoss {
    pub resource: ResourceType,
    pub amount: u64,
    pub stage: InventoryLossStage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryTurnReport {
    /// Main inventory and pending buffer together; excludes Labour.
    pub opening_stock: MaterialTotals,
    pub closing_stock: MaterialTotals,
    pub produced: MaterialTotals,
    pub returned: MaterialTotals,
    pub population_consumed: MaterialTotals,
    pub project_consumed: MaterialTotals,
    pub production_consumed: MaterialTotals,
    pub losses: Vec<InventoryLoss>,
}

impl InventoryTurnReport {
    pub(crate) fn snapshot(
        main: &ResourceInventory,
        pending: &ResourceInventory,
    ) -> MaterialTotals {
        let mut stock = MaterialTotals::new();
        for (&resource, &amount) in main.resources().iter().chain(pending.resources()) {
            if resource != ResourceType::Labour && amount > 0 {
                *stock.entry(resource).or_default() += u64::from(amount);
            }
        }
        stock
    }

    pub(crate) fn from_allocations(
        opening_stock: MaterialTotals,
        allocations: &AllocationResult,
    ) -> Self {
        let mut report = Self {
            opening_stock,
            ..Self::default()
        };
        for (recipient, grants) in allocations {
            let consumed = match recipient {
                AllocationRecipient::Population => &mut report.population_consumed,
                AllocationRecipient::Project(_) => &mut report.project_consumed,
                AllocationRecipient::Building(_) => &mut report.production_consumed,
            };
            for (&resource, &amount) in grants {
                if resource != ResourceType::Labour && amount > 0 {
                    *consumed.entry(resource).or_default() += u64::from(amount);
                }
            }
        }
        report
    }

    pub(crate) fn record_production(
        &mut self,
        produced: &HashMap<ResourceType, u64>,
        returned: &HashMap<ResourceType, u64>,
    ) {
        self.produced = produced
            .iter()
            .filter(|(res, amount)| **res != ResourceType::Labour && **amount > 0)
            .map(|(&res, &amount)| (res, amount))
            .collect();
        self.returned = returned
            .iter()
            .filter(|(res, amount)| **res != ResourceType::Labour && **amount > 0)
            .map(|(&res, &amount)| (res, amount))
            .collect();
        for (&resource, &amount) in &self.returned {
            let granted = self.production_consumed.entry(resource).or_default();
            *granted = granted
                .checked_sub(amount)
                .expect("returned materials cannot exceed building grants");
        }
    }

    /// Checks each material separately; internal returns are not new production.
    pub fn is_balanced(&self) -> bool {
        let resources: BTreeSet<_> = [
            &self.opening_stock,
            &self.closing_stock,
            &self.produced,
            &self.population_consumed,
            &self.project_consumed,
            &self.production_consumed,
        ]
        .into_iter()
        .flat_map(|totals| totals.keys().copied())
        .chain(self.losses.iter().map(|loss| loss.resource))
        .filter(|resource| *resource != ResourceType::Labour)
        .collect();
        resources.into_iter().all(|resource| {
            let amount =
                |totals: &MaterialTotals| u128::from(totals.get(&resource).copied().unwrap_or(0));
            let losses: u128 = self
                .losses
                .iter()
                .filter(|loss| loss.resource == resource)
                .map(|loss| u128::from(loss.amount))
                .sum();
            amount(&self.opening_stock) + amount(&self.produced)
                == amount(&self.closing_stock)
                    + amount(&self.population_consumed)
                    + amount(&self.project_consumed)
                    + amount(&self.production_consumed)
                    + losses
        })
    }
}

/// Results of one settlement turn. Pending projects produce no outcome entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnReport {
    pub consumption: ConsumptionResult,
    pub projects: Vec<ProjectOutcome>,
    pub inventory: InventoryTurnReport,
}
