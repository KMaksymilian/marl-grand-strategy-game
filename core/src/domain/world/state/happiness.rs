//! Adapts nation policy and settlement state to the configured happiness rules.
use super::{World, WorldError};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::settlement::happiness::{HappinessInputs, HappinessOutcome, HappinessRules};

impl World {
    pub fn set_happiness_rules(&mut self, rules: HappinessRules) {
        self.happiness_rules = Some(rules);
    }

    /// Supplies the national weariness value used by all of the nation's settlements.
    pub fn set_war_weariness(
        &mut self,
        nation_id: NationId,
        weariness: u8,
    ) -> Result<(), WorldError> {
        if weariness > 100 {
            return Err(WorldError::InvalidWarWeariness(weariness));
        }
        self.nation_mut(nation_id)?.set_war_weariness(weariness);
        Ok(())
    }

    pub fn settlement_happiness(
        &self,
        id: SettlementId,
    ) -> Result<Option<HappinessOutcome>, WorldError> {
        let settlement = self
            .settlement(id)
            .ok_or(WorldError::MissingSettlement(id))?;
        Ok(self.happiness_rules.as_ref().map(|rules| {
            let ownership = self
                .nations
                .ownership(id)
                .expect("registered settlement has an owner");
            let nation = self
                .nation(ownership.nation())
                .expect("owner is registered");
            let decrees = self.decree_effects(id);
            rules.evaluate(HappinessInputs {
                tax_rate: nation.tax_rate(),
                monument_happiness: settlement.monument_happiness_bonus(),
                monument_multiplier: decrees.monument_happiness_multiplier,
                decree_happiness: decrees.happiness,
                war_weariness: nation.war_weariness(),
                conquest_turns: ownership.conquest_turns(),
                capital_loss_turns: nation.capital_loss_turns(),
            })
        }))
    }
}
