//! Production tax of one settlement in the global turn (gameplay specification §6).
//!
//! The coordinator calls this for every settlement in ascending ID, after the
//! Production phase and before EconomicUpdates: the tax is taken from this
//! turn's gross output before it is stored, so warehouse losses never reduce
//! the nation's share. In short: `tax = rate × produced`, summed per nation.
use super::{World, WorldError};
use crate::domain::ids::SettlementId;
use crate::domain::nation::taxation::{MaterialTotals, SettlementTaxReport, TaxStatus};
use crate::domain::settlement::ProducedTurn;

/// What one settlement's taxation moved, for the settlement's and the
/// nation's reports.
#[derive(Debug, Clone, PartialEq)]
pub struct TaxCollection {
    pub report: SettlementTaxReport,
    /// Production tax stored in the national stock.
    pub stock_received: MaterialTotals,
    /// Production tax above the stock's shared capacity; lost.
    pub stock_lost: MaterialTotals,
}

impl World {
    /// Takes the owner's production tax from the settlement's gross output:
    ///
    /// 1. status and effective rate: nothing under attack, half while freshly
    ///    conquered, otherwise the nation's rate;
    /// 2. each produced material separately, rounded down, out of the output;
    /// 3. into the national stock up to its shared capacity; the rest is lost.
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
        let nation = self.nation_mut(nation_id)?;

        // 1. Which share of the rate this settlement pays.
        let status = if ownership.is_under_attack() {
            TaxStatus::UnderAttack
        } else if ownership.is_freshly_conquered() {
            TaxStatus::FreshlyConquered
        } else {
            TaxStatus::Regular
        };
        let nominal_rate = nation.tax_rate();
        let effective_rate = status.effective_rate(nominal_rate);

        // 2. Settlement side: the tax leaves this turn's gross output.
        let production_tax = produced.levy_production_tax(effective_rate);

        // ECONOMY (gold income, §4.4 and §6): once settlements earn gold, take
        // `income × effective_rate` from it here, deposit it with
        // `nation.deposit_gold` and report it in `SettlementTaxReport`.

        // 3. Nation side: the stock accepts what fits its shared capacity.
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
        for (resource, &amount) in &production_tax {
            let offered = u32::try_from(amount).unwrap_or(u32::MAX);
            let accepted = u64::from(nation.deposit_resource(resource.clone(), offered).accepted);
            if accepted > 0 {
                collection.stock_received.insert(resource.clone(), accepted);
            }
            if amount > accepted {
                collection
                    .stock_lost
                    .insert(resource.clone(), amount - accepted);
            }
        }
        Ok(collection)
    }
}
