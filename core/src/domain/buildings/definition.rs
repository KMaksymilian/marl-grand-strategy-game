use crate::domain::economy::resource::ResourceType;
use crate::domain::settlement::projects::ProjectCost;
use crate::domain::settlement::stat::StatType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum BuildingBehavior {
    Production {
        inputs: Vec<(ResourceType, u32)>,
        outputs: Vec<(ResourceType, u32)>,
    },
    StatModifier {
        stat: StatType,
        base_amount: u32,
        per_worker_amount: u32,
    },
}

#[derive(Debug)]
pub struct BuildingDefinition {
    pub id: String,
    pub name: String,
    pub tier: u8,
    pub max_workers: u32,
    pub behaviors: Vec<BuildingBehavior>,
    pub construction_cost: ProjectCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionError {
    InputOverflow(ResourceType),
    OutputOverflow(ResourceType),
    LabourInRecipe,
    StatOverflow,
}
impl BuildingDefinition {
    pub fn validate(&self) -> Result<(), DefinitionError> {
        let mut inputs = HashMap::<ResourceType, u32>::new();
        let mut outputs = HashMap::<ResourceType, u32>::new();
        for behavior in &self.behaviors {
            match behavior {
                BuildingBehavior::Production {
                    inputs: recipe_inputs,
                    outputs: recipe_outputs,
                } => {
                    for (entries, totals, input) in [
                        (recipe_inputs, &mut inputs, true),
                        (recipe_outputs, &mut outputs, false),
                    ] {
                        for &(resource, amount) in entries {
                            if resource == ResourceType::Labour {
                                return Err(DefinitionError::LabourInRecipe);
                            }
                            let total = totals.entry(resource).or_default();
                            *total = total.checked_add(amount).ok_or(if input {
                                DefinitionError::InputOverflow(resource)
                            } else {
                                DefinitionError::OutputOverflow(resource)
                            })?;
                        }
                    }
                }
                BuildingBehavior::StatModifier {
                    base_amount,
                    per_worker_amount,
                    ..
                } => {
                    per_worker_amount
                        .checked_mul(self.max_workers)
                        .and_then(|value| value.checked_add(*base_amount))
                        .ok_or(DefinitionError::StatOverflow)?;
                }
            }
        }
        Ok(())
    }
}
