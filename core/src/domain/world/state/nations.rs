//! Nation ownership, capital succession, extinction and national transfers.
//! World is the only authority that changes which nation owns a settlement.
//!
//! Why here and not in `Nation`: these rules need data from both aggregates.
//! Capital succession reads settlement populations, subsidies write to a
//! settlement's warehouse and treasury, and extinction removes a nation from
//! the registry. World owns both registries, so it is the one place that can
//! keep them consistent. Every method checks all conditions before it changes
//! anything, so a returned error always means "nothing happened".
use super::{World, WorldError};
use crate::domain::economy::{InventoryAddResult, ResourceType};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::registry::NationRegistry;
use crate::domain::nation::{Nation, TaxRate};
use serde::Serialize;
use std::collections::BTreeMap;

/// What one change of ownership did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnershipChange {
    pub settlement_id: SettlementId,
    /// Previous owner; None when the settlement was independent.
    pub from: Option<NationId>,
    pub to: NationId,
    /// The previous owner's new capital when it lost its capital and survives.
    pub new_capital: Option<SettlementId>,
    /// The previous owner had no settlement left and was removed.
    pub extinct: bool,
    /// Gold that vanished: the whole treasury of an extinct nation.
    pub gold_lost: u32,
    /// Materials that vanished: stock above the previous owner's reduced
    /// capacity, or the whole stock of an extinct nation.
    pub stock_lost: BTreeMap<ResourceType, u32>,
}

/// What one subsidy moved from a nation to one of its settlements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubsidyTransfer {
    pub nation_id: NationId,
    pub settlement_id: SettlementId,
    pub gold: u32,
    /// Materials stored in the settlement's warehouse.
    pub delivered: BTreeMap<ResourceType, u32>,
    /// Materials that did not fit the warehouse and stayed in the national stock.
    pub returned: BTreeMap<ResourceType, u32>,
}

/// Read access.
impl World {
    pub fn nations(&self) -> &NationRegistry {
        &self.nations
    }

    pub fn nation(&self, id: NationId) -> Option<&Nation> {
        self.nations.get(id)
    }

    /// The owning nation, or None for an independent settlement.
    pub fn settlement_owner(&self, settlement_id: SettlementId) -> Option<NationId> {
        self.nations.owner_of(settlement_id)
    }

    /// The owner of a settled province; empty provinces belong to nobody.
    pub fn province_owner(&self, province_id: usize) -> Result<Option<NationId>, WorldError> {
        Ok(self
            .province_settlement(province_id)?
            .and_then(|settlement_id| self.nations.owner_of(settlement_id)))
    }

    /// Settlements owned by the nation, ascending ID.
    pub fn nation_settlements(&self, nation_id: NationId) -> Result<Vec<SettlementId>, WorldError> {
        self.nations
            .get(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(self.nations.settlements_of(nation_id))
    }

    /// Per-material capacity of the national stock.
    pub fn nation_stock_capacity(&self, nation_id: NationId) -> Result<u32, WorldError> {
        self.nations
            .get(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(self.nations.stock_capacity(nation_id))
    }

    /// The most populous settlement, lowest ID on ties.
    fn largest_settlement(&self, candidates: &[SettlementId]) -> Option<SettlementId> {
        // `max_by_key` keeps the larger key; `Reverse(id)` makes a lower ID the
        // larger key, so equal populations resolve to the lowest ID. The result
        // is deterministic regardless of iteration order.
        candidates.iter().copied().max_by_key(|&id| {
            let population = self
                .settlement(id)
                .map_or(0, |settlement| settlement.population());
            (population, std::cmp::Reverse(id))
        })
    }
}

/// Ownership changes.
impl World {
    /// Creates a nation that owns an independent settlement; the settlement
    /// becomes its capital.
    pub fn found_nation(&mut self, capital: SettlementId) -> Result<NationId, WorldError> {
        if self.settlement(capital).is_none() {
            return Err(WorldError::MissingSettlement(capital));
        }
        if let Some(nation_id) = self.nations.owner_of(capital) {
            return Err(WorldError::SettlementAlreadyOwned {
                settlement_id: capital,
                nation_id,
            });
        }
        // `found` records the nation and its first ownership entry together;
        // it fails only when all u32 nation IDs have been used.
        self.nations
            .found(capital)
            .ok_or(WorldError::NationIdExhausted)
    }

    /// Gives a settlement (independent or owned) to another nation. The previous
    /// owner gets a new capital when it lost its capital, loses stock above its
    /// reduced capacity, and is removed with its treasury and stock when nothing
    /// is left. Fails without changes for unknown IDs or the current owner.
    pub fn transfer_settlement(
        &mut self,
        settlement_id: SettlementId,
        to: NationId,
    ) -> Result<OwnershipChange, WorldError> {
        // 1. Validate everything first: after this block the transfer cannot fail.
        if self.settlement(settlement_id).is_none() {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        if self.nations.get(to).is_none() {
            return Err(WorldError::MissingNation(to));
        }
        if self.nations.owner_of(settlement_id) == Some(to) {
            return Err(WorldError::SettlementAlreadyOwned {
                settlement_id,
                nation_id: to,
            });
        }
        // 2. The single ownership record changes; `set_owner` returns the old owner.
        let from = self.nations.set_owner(settlement_id, to);
        let mut change = OwnershipChange {
            settlement_id,
            from,
            to,
            new_capital: None,
            extinct: false,
            gold_lost: 0,
            stock_lost: BTreeMap::new(),
        };
        // 3. An independent settlement had no previous owner to update.
        let Some(previous) = from else {
            return Ok(change);
        };
        // 4. The previous owner lost its last settlement: it leaves the game.
        //    Its treasury and stock have nowhere to go, so they are reported as lost.
        let remaining = self.nations.settlements_of(previous);
        if remaining.is_empty() {
            let mut nation = self
                .nations
                .remove(previous)
                .expect("previous owner exists");
            change.extinct = true;
            change.gold_lost = nation.take_treasury();
            change.stock_lost = nation.stock_mut().take_all();
            return Ok(change);
        }
        // 5. The previous owner survives. Its capacity is read after `set_owner`,
        //    so it already counts one settlement less.
        let capacity = self.nations.stock_capacity(previous);
        let new_capital = self.largest_settlement(&remaining);
        let nation = self
            .nations
            .get_mut(previous)
            .expect("previous owner exists");
        // 6. Losing the capital moves it to the largest settlement and starts the
        //    capital-loss penalty (gameplay spec 5.1 and 4.2).
        if nation.capital() == settlement_id {
            let capital = new_capital.expect("remaining settlements exist");
            nation.replace_lost_capital(capital);
            change.new_capital = Some(capital);
        }
        // 7. Stock above the reduced capacity is discarded and reported.
        change.stock_lost = nation.stock_mut().enforce_capacity(capacity);
        Ok(change)
    }
}

/// National treasury, stock and policy.
impl World {
    /// Adds gold to the treasury (scenario setup, taxes, customs); returns the
    /// accepted amount.
    pub fn deposit_nation_gold(
        &mut self,
        nation_id: NationId,
        amount: u32,
    ) -> Result<u32, WorldError> {
        Ok(self.nation_mut(nation_id)?.deposit_gold(amount))
    }

    /// Adds materials to the national stock up to its capacity; the rest is lost.
    pub fn deposit_nation_resource(
        &mut self,
        nation_id: NationId,
        resource: ResourceType,
        amount: u32,
    ) -> Result<InventoryAddResult, WorldError> {
        let capacity = self.nation_stock_capacity(nation_id)?;
        Ok(self
            .nation_mut(nation_id)?
            .stock_mut()
            .add(resource, amount, capacity))
    }

    pub(crate) fn set_tax_rate(
        &mut self,
        nation_id: NationId,
        rate: TaxRate,
    ) -> Result<(), WorldError> {
        self.nation_mut(nation_id)?.set_tax_rate(rate);
        Ok(())
    }

    /// Moves gold and materials from the nation to one of its settlements, all or
    /// nothing. Materials that do not fit the warehouse stay in the national stock.
    pub(crate) fn subsidize(
        &mut self,
        nation_id: NationId,
        settlement_id: SettlementId,
        gold: u32,
        resources: &BTreeMap<ResourceType, u32>,
    ) -> Result<SubsidyTransfer, WorldError> {
        // Phase 1: checks only, no mutation. The application validator performs
        // the same checks per action; these repeat them so World stays safe even
        // when called directly.
        let settlement_gold = self
            .settlement(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?
            .gold();
        if self.nations.owner_of(settlement_id) != Some(nation_id) {
            return Err(WorldError::SettlementNotOwned {
                settlement_id,
                nation_id,
            });
        }
        let nation = self
            .nations
            .get(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        let affordable = nation.treasury() >= gold
            && resources.iter().all(|(&resource, &amount)| {
                resource != ResourceType::Labour && nation.stock().amount(resource) >= amount
            });
        // The settlement's gold counter is a u32: refuse rather than overflow.
        if !affordable || gold > u32::MAX - settlement_gold {
            return Err(WorldError::InsufficientNationalFunds(nation_id));
        }

        // Phase 2: move everything. The asserts cannot fire after phase 1.
        let nation = self.nations.get_mut(nation_id).expect("checked above");
        assert!(nation.withdraw_gold(gold));
        for (&resource, &amount) in resources {
            assert!(nation.stock_mut().take(resource, amount));
        }
        let settlement = self
            .settlements
            .get_mut(settlement_id)
            .expect("checked above");
        settlement.deposit_gold(gold);
        let mut transfer = SubsidyTransfer {
            nation_id,
            settlement_id,
            gold,
            delivered: BTreeMap::new(),
            returned: BTreeMap::new(),
        };
        // The warehouse has one shared capacity; what it rejects is not lost but
        // sent back to the national stock below, so the subsidy conserves goods.
        for (&resource, &amount) in resources.iter().filter(|(_, amount)| **amount > 0) {
            let InventoryAddResult { accepted, rejected } = settlement.deposit(resource, amount);
            if accepted > 0 {
                transfer.delivered.insert(resource, accepted);
            }
            if rejected > 0 {
                transfer.returned.insert(resource, rejected);
            }
        }
        let nation = self.nations.get_mut(nation_id).expect("checked above");
        for (&resource, &amount) in &transfer.returned {
            // The units just left this stock, so they always fit again.
            let back = nation.stock_mut().add(resource, amount, u32::MAX);
            debug_assert_eq!(back.rejected, 0);
        }
        Ok(transfer)
    }

    /// Counts down national timers at the end of a global turn.
    pub(crate) fn advance_nations_turn(&mut self) {
        for nation in self.nations.iter_mut() {
            nation.advance_turn();
        }
    }

    fn nation_mut(&mut self, nation_id: NationId) -> Result<&mut Nation, WorldError> {
        self.nations
            .get_mut(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))
    }
}
