//! Production tax of one settlement in the global turn (gameplay specification §6).
//!
//! Runs after the Production phase and before EconomicUpdates: the tax is taken
//! from this turn's gross output before it is stored, so warehouse losses never
//! reduce the nation's share. Gold is not taxed yet: the gold tax applies to the
//! settlement's gold income (§4.4), which does not exist in the economy yet.
use super::{World, WorldError};
use crate::domain::ids::SettlementId;
use crate::domain::nation::TaxRate;
use crate::domain::nation::taxation::{MaterialTotals, SettlementTaxReport, TaxStatus};
use crate::domain::settlement::ProducedTurn;

/// What one settlement's taxation moved, for the settlement's and the
/// nation's reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxCollection {
    pub report: SettlementTaxReport,
    /// Production tax stored in the national stock.
    pub stock_received: MaterialTotals,
    /// Production tax above the per-material capacity; lost.
    pub stock_lost: MaterialTotals,
}

impl World {
    /// Takes the owner's production tax from the settlement's gross output:
    ///
    /// 1. effective rate: none under attack, half while freshly conquered,
    ///    otherwise the nation's rate;
    /// 2. each produced material separately, rounded down;
    /// 3. into the national stock up to its per-material capacity; the rest is lost.
    pub(crate) fn collect_taxes(
        &mut self,
        settlement_id: SettlementId,
        produced: &mut ProducedTurn,
    ) -> Result<TaxCollection, WorldError> {
        let ownership = *self
            .nations
            .ownership(settlement_id)
            .ok_or(WorldError::MissingSettlement(settlement_id))?;
        let nation_id = ownership.nation();
        let capacity = self.nations.stock_capacity(nation_id);
        let nation = self
            .nations
            .get_mut(nation_id)
            .ok_or(WorldError::MissingNation(nation_id))?;

        // 1. Which share of the rate this settlement pays.
        let nominal_rate = nation.tax_rate();
        let (status, effective_rate) = if ownership.is_under_attack() {
            (TaxStatus::UnderAttack, TaxRate::ZERO)
        } else if ownership.is_freshly_conquered() {
            (TaxStatus::FreshlyConquered, nominal_rate.halved())
        } else {
            (TaxStatus::Regular, nominal_rate)
        };

        // 2. Settlement side: the tax leaves this turn's gross output.
        let production_tax = produced.levy_production_tax(effective_rate);

        // 3. Nation side: the stock accepts what fits.
        let mut collection = TaxCollection {
            report: SettlementTaxReport {
                nation_id,
                status,
                nominal_rate,
                effective_rate,
                production_tax: production_tax.clone(),
            },
            stock_received: MaterialTotals::new(),
            stock_lost: MaterialTotals::new(),
        };
        for (&resource, &amount) in &production_tax {
            let offered = u32::try_from(amount).unwrap_or(u32::MAX);
            let added = nation.stock_mut().add(resource, offered, capacity);
            if added.accepted > 0 {
                collection
                    .stock_received
                    .insert(resource, u64::from(added.accepted));
            }
            let lost = amount - u64::from(added.accepted);
            if lost > 0 {
                collection.stock_lost.insert(resource, lost);
            }
        }
        Ok(collection)
    }
}
