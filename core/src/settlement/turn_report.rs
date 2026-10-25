use crate::economy::allocation::{AllocationRecipient, AllocationResult};
use crate::economy::{ResourceInventory, ResourceType};
pub use crate::settlement::population::ConsumptionResult;
use crate::settlement::projects::ProjectOutcome;
use crate::trade::caravan::CaravanOutcome;
use crate::trade::execution::TradeExecution;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type MaterialTotals = BTreeMap<ResourceType, u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryLossStage {
    Returns,
    ProductionBuffer,
    Warehouse,
    /// Goods coming back from the trade book or a returning caravan.
    TradeReturns,
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
    /// Materials moved out of `main` into the market book or caravans.
    pub trade_reserved: MaterialTotals,
    /// Materials coming back from trade (purchases, unsold stock, caravans), before losses.
    pub trade_received: MaterialTotals,
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
            &self.trade_reserved,
            &self.trade_received,
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
            amount(&self.opening_stock) + amount(&self.produced) + amount(&self.trade_received)
                == amount(&self.closing_stock)
                    + amount(&self.population_consumed)
                    + amount(&self.project_consumed)
                    + amount(&self.production_consumed)
                    + amount(&self.trade_reserved)
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
    pub trade: TradeTurnReport,
}

/// A trade made at this settlement's market with a visiting caravan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketTrade {
    pub caravan_sender_id: u32,
    /// Seen from this settlement.
    pub execution: TradeExecution,
}

/// A caravan funded and sent out this turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchedCaravan {
    pub receiver_id: u32,
    pub cargo: MaterialTotals,
    pub gold: u64,
}

/// A caravan unloaded this turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedCaravan {
    pub receiver_id: u32,
    pub outcome: CaravanOutcome,
    /// Seen from this settlement.
    pub trades: Vec<TradeExecution>,
    /// Goods brought home (purchases and unsold stock), before warehouse losses.
    pub cargo: MaterialTotals,
    pub gold: u64,
}

/// Trade activity of one settlement turn.
///
/// Material flows are also part of `InventoryTurnReport` (`trade_reserved`,
/// `trade_received` and losses with stage `TradeReturns`), so `is_balanced()`
/// covers trade. Gold is reported only here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TradeTurnReport {
    /// Stock and gold reserved by the local market book.
    pub market_reserved: MaterialTotals,
    pub market_gold_reserved: u64,
    pub market_trades: Vec<MarketTrade>,
    /// Purchases and unsold stock collected from the market book.
    pub market_bought: MaterialTotals,
    pub market_unsold: MaterialTotals,
    pub caravans_dispatched: Vec<DispatchedCaravan>,
    /// Receiver IDs of caravan orders skipped because nothing could be funded.
    pub unfunded_caravan_orders: Vec<u32>,
    pub caravans_returned: Vec<ReturnedCaravan>,
    /// All materials returned from trade this turn, before warehouse losses.
    pub received: MaterialTotals,
    /// Gold returned from the market book and returning caravans.
    pub gold_received: u64,
    /// Returned materials rejected by a full warehouse.
    pub lost: MaterialTotals,
}

impl TradeTurnReport {
    /// Executions at the local market plus those of caravans unloaded this turn.
    pub fn trade_count(&self) -> usize {
        self.market_trades.len()
            + self
                .caravans_returned
                .iter()
                .map(|caravan| caravan.trades.len())
                .sum::<usize>()
    }

    /// Materials taken from stock this turn by the market and new caravans.
    pub fn reserved(&self) -> MaterialTotals {
        let mut reserved = self.market_reserved.clone();
        for caravan in &self.caravans_dispatched {
            for (&resource, &amount) in &caravan.cargo {
                *reserved.entry(resource).or_default() += amount;
            }
        }
        reserved
    }

    /// Gold taken from the treasury this turn by the market and new caravans.
    pub fn gold_reserved(&self) -> u64 {
        self.market_gold_reserved
            + self
                .caravans_dispatched
                .iter()
                .map(|caravan| caravan.gold)
                .sum::<u64>()
    }
}
