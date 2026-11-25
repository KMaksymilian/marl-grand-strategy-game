use crate::domain::buildings::effects::StatType;
use crate::domain::economy::ResourceType;
use crate::domain::economy::cost::ProjectCost;
use crate::domain::spatial::site::SiteRequirements;
use crate::domain::world::terrain::biome::Biome;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct CycleRecipe {
    pub inputs: Vec<(ResourceType, u32)>,
    pub outputs: Vec<(ResourceType, u32)>,
}

#[derive(Debug)]
pub struct CycleProduction {
    pub cycles_per_turn: u32,
    pub recipes: Vec<CycleRecipe>,
}

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
    site_requirements: SiteRequirements,
    cycle_production: Option<CycleProduction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    InputOverflow(ResourceType),
    OutputOverflow(ResourceType),
    LabourInRecipe,
    InvalidDepositResource(ResourceType),
    StatOverflow,
    /// `max_workers` above `BuildingDefinition::MAX_WORKERS`.
    TooManyWorkers,
    InvalidBiomeEfficiencies,
    InvalidCycleProduction,
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
            site_requirements: SiteRequirements::default(),
            cycle_production: None,
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

    pub fn requiring_deposit(mut self, resource: ResourceType) -> Result<Self, DefinitionError> {
        if resource == ResourceType::Labour {
            return Err(DefinitionError::InvalidDepositResource(resource));
        }
        self.site_requirements = self.site_requirements.requiring_deposit(resource);
        Ok(self)
    }

    pub fn required_deposit(&self) -> Option<ResourceType> {
        self.site_requirements
            .province_deposit()
            .or_else(|| self.site_requirements.tile_deposit())
            .cloned()
    }

    pub fn requiring_coast(mut self) -> Self {
        self.site_requirements = self.site_requirements.requiring_coast();
        self
    }

    pub fn requiring_province_deposit(
        mut self,
        resource: ResourceType,
    ) -> Result<Self, DefinitionError> {
        if resource == ResourceType::Labour {
            return Err(DefinitionError::InvalidDepositResource(resource));
        }
        self.site_requirements = self.site_requirements.requiring_province_deposit(resource);
        Ok(self)
    }

    pub fn requires_province_deposit(&self) -> bool {
        self.site_requirements.province_deposit().is_some()
    }

    pub fn with_biome_efficiencies(
        mut self,
        efficiencies: BTreeMap<Biome, u8>,
    ) -> Result<Self, DefinitionError> {
        if efficiencies.is_empty()
            || efficiencies.contains_key(&Biome::Ocean)
            || efficiencies
                .values()
                .any(|&value| value == 0 || value > 100)
        {
            return Err(DefinitionError::InvalidBiomeEfficiencies);
        }
        self.site_requirements.set_biome_efficiencies(efficiencies);
        Ok(self)
    }

    pub fn biome_efficiencies(&self) -> Option<&BTreeMap<Biome, u8>> {
        self.site_requirements.biome_efficiencies()
    }

    pub fn is_biome_restricted(&self) -> bool {
        self.biome_efficiencies().is_some()
    }

    pub fn biome_efficiency(&self, biome: Biome) -> Option<u8> {
        if biome == Biome::Ocean {
            return None;
        }
        self.biome_efficiencies()
            .map_or(Some(100), |efficiencies| efficiencies.get(&biome).copied())
    }

    pub fn with_cycle_production(
        mut self,
        cycles_per_turn: u32,
        recipes: Vec<CycleRecipe>,
    ) -> Result<Self, DefinitionError> {
        if cycles_per_turn == 0
            || self.max_workers == 0
            || recipes.is_empty()
            || self
                .behaviors
                .iter()
                .any(|behavior| matches!(behavior, BuildingBehavior::Production { .. }))
        {
            return Err(DefinitionError::InvalidCycleProduction);
        }
        for recipe in &recipes {
            if recipe.outputs.is_empty() {
                return Err(DefinitionError::InvalidCycleProduction);
            }
            for materials in [&recipe.inputs, &recipe.outputs] {
                let mut seen = HashSet::new();
                for (resource, amount) in materials {
                    if resource == &ResourceType::Labour
                        || *amount == 0
                        || !seen.insert(resource)
                        || amount.checked_mul(cycles_per_turn).is_none()
                    {
                        return Err(DefinitionError::InvalidCycleProduction);
                    }
                }
            }
        }
        self.cycle_production = Some(CycleProduction {
            cycles_per_turn,
            recipes,
        });
        Ok(self)
    }

    pub fn cycle_production(&self) -> Option<&CycleProduction> {
        self.cycle_production.as_ref()
    }

    pub fn requires_coast(&self) -> bool {
        self.site_requirements.requires_coast()
    }

    pub fn with_site_requirements(mut self, requirements: SiteRequirements) -> Self {
        self.site_requirements = requirements;
        self
    }

    pub fn site_requirements(&self) -> &SiteRequirements {
        &self.site_requirements
    }

    fn validate(&self) -> Result<(), DefinitionError> {
        if self.max_workers > Self::MAX_WORKERS {
            return Err(DefinitionError::TooManyWorkers);
        }
        let mut inputs = HashMap::<&ResourceType, u32>::new();
        let mut outputs = HashMap::<&ResourceType, u32>::new();
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
                        for (resource, amount) in entries {
                            if *resource == ResourceType::Labour {
                                return Err(DefinitionError::LabourInRecipe);
                            }
                            let total = totals.entry(resource).or_default();
                            *total = total.checked_add(*amount).ok_or_else(|| {
                                if input {
                                    DefinitionError::InputOverflow(resource.clone())
                                } else {
                                    DefinitionError::OutputOverflow(resource.clone())
                                }
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
