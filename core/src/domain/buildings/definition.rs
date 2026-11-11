use crate::domain::buildings::effects::StatType;
use crate::domain::economy::ResourceType;
use crate::domain::economy::cost::ProjectCost;
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

/// Immutable validated blueprint. All instances must pass through `new`.
/// ```compile_fail
/// use simulation_core::buildings::definition::BuildingDefinition;
/// fn mutate(definition: &mut BuildingDefinition) {
///     definition.max_workers = u32::MAX;
/// }
/// ```
#[derive(Debug)]
pub struct BuildingDefinition {
    id: String,
    name: String,
    tier: u8,
    max_workers: u32,
    behaviors: Vec<BuildingBehavior>,
    construction_cost: ProjectCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionError {
    InputOverflow(ResourceType),
    OutputOverflow(ResourceType),
    LabourInRecipe,
    StatOverflow,
    /// `max_workers` above `BuildingDefinition::MAX_WORKERS`.
    TooManyWorkers,
}
impl BuildingDefinition {
    /// Largest staff for which every count `0..=max_workers` maps to an exact
    /// `f32` building load (the integer range of `f32`).
    pub const MAX_WORKERS: u32 = 1 << 24;

    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        tier: u8,
        max_workers: u32,
        behaviors: Vec<BuildingBehavior>,
        construction_cost: ProjectCost,
    ) -> Result<Self, DefinitionError> {
        let definition = Self {
            id: id.into(),
            name: name.into(),
            tier,
            max_workers,
            behaviors,
            construction_cost,
        };
        definition.validate()?;
        Ok(definition)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn tier(&self) -> u8 {
        self.tier
    }
    pub fn max_workers(&self) -> u32 {
        self.max_workers
    }
    pub fn behaviors(&self) -> &[BuildingBehavior] {
        &self.behaviors
    }
    pub fn construction_cost(&self) -> &ProjectCost {
        &self.construction_cost
    }

    fn validate(&self) -> Result<(), DefinitionError> {
        if self.max_workers > Self::MAX_WORKERS {
            return Err(DefinitionError::TooManyWorkers);
        }
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
