//! Nation ownership, capital succession, extinction and national transfers.
//! World is the only authority that changes which nation owns a settlement.
//!
//! Why here and not in `Nation`: these rules need both aggregates. Capital
//! succession reads settlement populations, subsidies write to a settlement's
//! warehouse and gold, and extinction removes a nation from the registry. World
//! owns both registries, so it is the one place that keeps them consistent.
//! Every method checks all conditions before it changes anything, so an
//! error always means that nothing happened.
use super::{World, WorldError};
use crate::domain::economy::{InventoryAddResult, ResourceType};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::Nation;
use crate::domain::nation::registry::NationRegistry;
use serde::Serialize;
use std::collections::BTreeMap;

/// What one change of ownership did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnershipChange {
    pub settlement_id: SettlementId,
    pub from: NationId,
    pub to: NationId,
    /// The previous owner's new capital when it lost its capital and survives.
    pub new_capital: Option<SettlementId>,
    /// The previous owner had no settlement left and was removed.
    pub extinct: bool,
    /// Gold that vanished: the whole treasury of an extinct nation.
    pub gold_lost: u32,
    /// Materials that vanished: the whole stock of an extinct nation.
    pub stock_lost: BTreeMap<ResourceType, u32>,
}

/// What one subsidy moved from a nation to one of its settlements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubsidyTransfer {
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

    /// The owning nation. Every registered settlement has one; None means the
    /// settlement ID is unknown.
    pub fn settlement_owner(&self, settlement_id: SettlementId) -> Option<NationId> {
        self.nations.owner_of(settlement_id)
    }

    /// The owner of a settled province; an empty province belongs to nobody.
    pub fn province_owner(&self, province_id: usize) -> Result<Option<NationId>, WorldError> {
        Ok(self
            .province_settlement(province_id)?
            .and_then(|settlement_id| self.nations.owner_of(settlement_id)))
    }

    /// Settlements owned by the nation, ascending ID.
    pub fn nation_settlements(&self, nation_id: NationId) -> Result<Vec<SettlementId>, WorldError> {
        self.nation(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(self.nations.settlements_of(nation_id))
    }

    /// Capacity of the national stock, shared by all materials.
    pub fn nation_stock_capacity(&self, nation_id: NationId) -> Result<u32, WorldError> {
        self.nation(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;
        Ok(self.nations.stock_capacity(nation_id))
    }

    /// The most populous settlement, lowest ID on ties.
    fn largest_settlement(&self, candidates: &[SettlementId]) -> Option<SettlementId> {
        // `max_by_key` keeps the larger key; `Reverse(id)` makes a lower ID the
        // larger key, so equal populations resolve to the lowest ID.
        candidates.iter().copied().max_by_key(|&id| {
            let population = self
                .settlement(id)
                .map_or(0, |settlement| settlement.population());
            (population, std::cmp::Reverse(id))
        })
    }
}

/// Ownership changes. Nations are founded when a settlement registers without
/// joining an existing nation, so there is no separate founding method.
impl World {
    /// Gives a settlement to another nation (the war system will call this when
    /// a settlement is conquered, §11.5). The previous owner gets a new capital
    /// when it lost its capital and is removed with its treasury and stock when
    /// nothing is left. Its stock capacity shrinks, but no stock is discarded. Fails without
    /// changes for unknown IDs or when `to` already owns the settlement.
    pub fn transfer_settlement(
        &mut self,
        settlement_id: SettlementId,
        to: NationId,
    ) -> Result<OwnershipChange, WorldError> {
        // 1. Validate everything first: after this block the transfer cannot fail.
        if self.settlement(settlement_id).is_none() {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        if self.nation(to).is_none() {
            return Err(WorldError::MissingNation(to));
        }
        if self.nations.owner_of(settlement_id) == Some(to) {
            return Err(WorldError::SettlementAlreadyOwned {
                settlement_id,
                nation_id: to,
            });
        }
        // 2. The single ownership record changes; both stock capacities follow.
        //    The settlement starts the conquest penalty (half tax, §6).
        let from = self
            .nations
            .capture(settlement_id, to)
            .expect("every registered settlement has an owner");
        let mut change = OwnershipChange {
            settlement_id,
            from,
            to,
            new_capital: None,
            extinct: false,
            gold_lost: 0,
            stock_lost: BTreeMap::new(),
        };
        // 3. The previous owner lost its last settlement: it leaves the game,
        //    and its treasury and stock have nowhere to go.
        let remaining = self.nations.settlements_of(from);
        if remaining.is_empty() {
            let mut nation = self.nations.remove(from).expect("previous owner exists");
            change.extinct = true;
            change.gold_lost = nation.take_treasury();
            change.stock_lost = nation.take_stock();
            // Its relations, proposals and reputation leave the game with it.
            self.diplomacy.remove_nation(from);
            return Ok(change);
        }
        // 4. The previous owner survives.
        let new_capital = self.largest_settlement(&remaining);
        let nation = self.nations.get_mut(from).expect("previous owner exists");
        // 5. Losing the capital moves it and starts the capital-loss penalty.
        if nation.capital() == settlement_id {
            let capital = new_capital.expect("remaining settlements exist");
            nation.replace_lost_capital(capital);
            change.new_capital = Some(capital);
        }
        Ok(change)
    }
}

/// National treasury, stock and policy.
impl World {
    /// Adds gold to the treasury (scenario setup; later taxes and customs);
    /// returns the accepted amount.
    pub fn deposit_nation_gold(
        &mut self,
        nation_id: NationId,
        amount: u32,
    ) -> Result<u32, WorldError> {
        Ok(self.nation_mut(nation_id)?.deposit_gold(amount))
    }

    /// Adds materials to the national stock up to its shared capacity; the rest
    /// is lost.
    pub fn deposit_nation_resource(
        &mut self,
        nation_id: NationId,
        resource: ResourceType,
        amount: u32,
    ) -> Result<InventoryAddResult, WorldError> {
        Ok(self
            .nation_mut(nation_id)?
            .deposit_resource(resource, amount))
    }

    /// `rate` must be a valid fraction (`Nation::is_valid_tax_rate`).
    pub(crate) fn set_tax_rate(
        &mut self,
        nation_id: NationId,
        rate: f32,
    ) -> Result<(), WorldError> {
        self.nation_mut(nation_id)?.set_tax_rate(rate);
        Ok(())
    }

    /// Moves gold and materials from the nation to one of its settlements, all
    /// or nothing. Materials that do not fit the warehouse stay in the national
    /// stock, so a subsidy neither creates nor destroys goods.
    pub(crate) fn subsidize(
        &mut self,
        nation_id: NationId,
        settlement_id: SettlementId,
        gold: u32,
        resources: &BTreeMap<ResourceType, u32>,
    ) -> Result<SubsidyTransfer, WorldError> {
        // Phase 1: checks only. The application validator performs the same
        // checks per action; these repeat them so World stays safe on its own.
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
            .nation(nation_id)
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
            assert!(nation.withdraw_resource(resource, amount));
        }
        let settlement = self
            .settlements
            .get_mut(settlement_id)
            .expect("checked above");
        settlement.deposit_gold(gold);
        let mut transfer = SubsidyTransfer {
            gold,
            delivered: BTreeMap::new(),
            returned: BTreeMap::new(),
        };
        for (&resource, &amount) in resources.iter().filter(|(_, amount)| **amount > 0) {
            let InventoryAddResult { accepted, rejected } = settlement.deposit(resource, amount);
            if accepted > 0 {
                transfer.delivered.insert(resource, accepted);
            }
            if rejected > 0 {
                transfer.returned.insert(resource, rejected);
            }
        }
        // What the warehouse rejected goes back; it just left this stock, so it fits.
        let nation = self.nations.get_mut(nation_id).expect("checked above");
        for (&resource, &amount) in &transfer.returned {
            let back = nation.deposit_resource(resource, amount);
            debug_assert_eq!(back.rejected, 0);
        }
        Ok(transfer)
    }

    /// Marks whether an enemy army stands in the settlement's province (§11.2):
    /// an attacked settlement pays no tax. The war system will set this from
    /// army positions; until then it is a scenario setting.
    pub fn set_settlement_under_attack(
        &mut self,
        settlement_id: SettlementId,
        attacked: bool,
    ) -> Result<(), WorldError> {
        if self.nations.set_under_attack(settlement_id, attacked) {
            Ok(())
        } else {
            Err(WorldError::MissingSettlement(settlement_id))
        }
    }

    /// Counts down national timers (capital-loss and conquest penalties) at
    /// the end of a global turn.
    pub(crate) fn advance_nations_turn(&mut self) {
        self.nations.advance_turn();
        self.diplomacy.advance_turn();
    }

    pub(super) fn nation_mut(&mut self, nation_id: NationId) -> Result<&mut Nation, WorldError> {
        self.nations
            .get_mut(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))
    }
}
