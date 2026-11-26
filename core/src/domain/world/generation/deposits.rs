//! Seeded ore focuses sampled by province and grown into connected land veins.
use crate::domain::economy::{ResourceCatalog, ResourceType};
use crate::domain::world::terrain::biome::Biome;
use crate::domain::world::{Position, World, WorldError};
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::collections::{BTreeMap, HashSet};

const LAND_BIOMES: [Biome; 8] = [
    Biome::Desert,
    Biome::Grassland,
    Biome::Forest,
    Biome::Rainforest,
    Biome::Hills,
    Biome::Taiga,
    Biome::Tundra,
    Biome::Snow,
];

#[derive(Debug, Clone)]
pub struct OreDepositRule {
    pub resource_id: String,
    /// Percent chances in Desert, Grassland, Forest, Rainforest, Hills, Taiga, Tundra, Snow.
    pub probabilities: [u8; 8],
}

impl OreDepositRule {
    pub fn chance_for(&self, biome: Biome) -> u8 {
        LAND_BIOMES
            .iter()
            .position(|&land| land == biome)
            .map_or(0, |index| self.probabilities[index])
    }
}

#[derive(Debug, Clone)]
pub struct DepositGenerationConfig {
    pub seed: u64,
    /// Maximum tiles per vein, including its focus. Must be positive.
    pub max_vein_tiles: u32,
    /// Maximum generated veins across the map in one generation pass.
    pub max_veins: u32,
    pub rules: Vec<OreDepositRule>,
}

impl Default for DepositGenerationConfig {
    fn default() -> Self {
        let limits = ResourceCatalog::standard().deposit_limits();
        Self {
            seed: 67,
            max_vein_tiles: limits.max_vein_tiles.get(),
            max_veins: limits.max_veins,
            rules: [
                ("coal", [0, 0, 25, 10, 5, 30, 0, 0]),
                ("iron_ore", [0, 0, 5, 0, 25, 5, 30, 20]),
                ("copper_ore", [30, 0, 0, 0, 20, 0, 5, 0]),
                ("tin_ore", [5, 0, 0, 0, 0, 0, 10, 20]),
            ]
            .into_iter()
            .map(|(id, probabilities)| OreDepositRule {
                resource_id: id.into(),
                probabilities,
            })
            .collect(),
        }
    }
}

impl DepositGenerationConfig {
    /// Standard generation skips ores absent from a custom content catalogue.
    /// Explicit rules supplied to `generate_deposits` require every material to exist.
    pub fn for_catalog(seed: u64, resources: &ResourceCatalog) -> Self {
        let limits = resources.deposit_limits();
        let mut config = Self {
            seed,
            max_vein_tiles: limits.max_vein_tiles.get(),
            max_veins: limits.max_veins,
            ..Self::default()
        };
        config.rules.retain(|rule| {
            resources
                .resolve(&rule.resource_id)
                .and_then(|resource| resources.get(resource))
                .is_some_and(|definition| definition.id() == rule.resource_id)
        });
        config
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedVein {
    pub province_id: usize,
    pub resource: ResourceType,
    pub focus: Position,
    pub tiles: Vec<Position>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DepositGenerationReport {
    pub veins: Vec<GeneratedVein>,
    /// Successful probability rolls with no unoccupied land left in that province.
    pub unplaced_focuses: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositGenerationError {
    ZeroMaxVeinTiles,
    InvalidProbability {
        resource_id: String,
        probability: u8,
    },
    UnknownResource(String),
    DuplicateResource(String),
    InvalidMap,
    Placement(WorldError),
}

#[derive(Default)]
struct ProvinceLand {
    positions: Vec<Position>,
    biome_counts: [usize; 8],
}

impl ProvinceLand {
    fn biome(&self) -> Biome {
        let mut most_common = 0;
        for index in 1..LAND_BIOMES.len() {
            if self.biome_counts[index] > self.biome_counts[most_common] {
                most_common = index;
            }
        }
        LAND_BIOMES[most_common]
    }
}

impl World {
    /// Add permanent deposits using independent streams for chance, focus and growth.
    /// Existing deposits are preserved. Configuration and map validation precede mutations.
    pub fn generate_deposits(
        &mut self,
        config: &DepositGenerationConfig,
    ) -> Result<DepositGenerationReport, DepositGenerationError> {
        let rules = validated_rules(config, self.resource_catalog())?;
        let mut provinces = province_land(self)?;
        let mut report = DepositGenerationReport::default();
        if config.max_veins == 0 {
            return Ok(report);
        }
        let mut candidates = Vec::new();
        for (&province_id, land) in &mut provinces {
            land.positions
                .retain(|&position| self.map().deposit_at(position).is_none());
            for (resource, rule) in &rules {
                let mut chance = stream(config.seed, province_id, resource, 0);
                if chance.random_range(0..100_u8) < rule.chance_for(land.biome()) {
                    candidates.push((province_id, resource));
                }
            }
        }
        let max_veins = config.max_veins as usize;
        if candidates.len() > max_veins {
            // Sample across the whole map instead of favoring the lowest province IDs.
            let mut selection = StdRng::seed_from_u64(config.seed ^ 0xa591_b8df_9472_1c03);
            candidates.shuffle(&mut selection);
        }
        for (province_id, resource) in candidates {
            if report.veins.len() == max_veins {
                break;
            }
            let available = &mut provinces
                .get_mut(&province_id)
                .expect("candidate province")
                .positions;
            if available.is_empty() {
                report.unplaced_focuses += 1;
                continue;
            }
            let mut focus_rng = stream(config.seed, province_id, resource, 1);
            let focus = available.swap_remove(focus_rng.random_range(0..available.len()));
            self.place_deposit(focus, resource.clone())
                .map_err(DepositGenerationError::Placement)?;
            report.veins.push(GeneratedVein {
                province_id,
                resource: resource.clone(),
                focus,
                tiles: vec![focus],
            });
        }
        // Reserve every focus before expansion, so growth cannot suppress another roll.
        for vein in &mut report.veins {
            self.grow_vein(vein, config)?;
        }
        Ok(report)
    }

    fn grow_vein(
        &mut self,
        vein: &mut GeneratedVein,
        config: &DepositGenerationConfig,
    ) -> Result<(), DepositGenerationError> {
        let mut rng = stream(config.seed, vein.province_id, &vein.resource, 2);
        let mut frontier = vec![vein.focus];
        let mut visited = HashSet::from([vein.focus]);
        let max_tiles = self
            .resource_catalog()
            .get(&vein.resource)
            .and_then(|definition| definition.max_vein_tiles())
            .map_or(config.max_vein_tiles, |limit| {
                limit.get().min(config.max_vein_tiles)
            });
        while !frontier.is_empty() && vein.tiles.len() < max_tiles as usize {
            let position = frontier.swap_remove(rng.random_range(0..frontier.len()));
            if position != vein.focus {
                if self.map().deposit_at(position).is_some() {
                    continue;
                }
                self.place_deposit(position, vein.resource.clone())
                    .map_err(DepositGenerationError::Placement)?;
                vein.tiles.push(position);
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let Some(x) = position.x.checked_add(dx) else {
                    continue;
                };
                let Some(y) = position.y.checked_add(dy) else {
                    continue;
                };
                let neighbor = Position { x, y };
                if visited.insert(neighbor)
                    && self.map().validate_building_position(neighbor).is_ok()
                    && self.map().deposit_at(neighbor).is_none()
                {
                    // Frontier tiles are candidates until selected, allowing branching.
                    frontier.push(neighbor);
                }
            }
        }
        Ok(())
    }
}

fn validated_rules<'a>(
    config: &'a DepositGenerationConfig,
    resources: &ResourceCatalog,
) -> Result<Vec<(ResourceType, &'a OreDepositRule)>, DepositGenerationError> {
    if config.max_vein_tiles == 0 {
        return Err(DepositGenerationError::ZeroMaxVeinTiles);
    }
    let mut seen = HashSet::new();
    let mut rules = Vec::new();
    for rule in &config.rules {
        if let Some(&probability) = rule.probabilities.iter().find(|&&chance| chance > 100) {
            return Err(DepositGenerationError::InvalidProbability {
                resource_id: rule.resource_id.clone(),
                probability,
            });
        }
        let resource = resources
            .resolve(&rule.resource_id)
            .ok_or_else(|| DepositGenerationError::UnknownResource(rule.resource_id.clone()))?;
        if !seen.insert(resource.clone()) {
            return Err(DepositGenerationError::DuplicateResource(
                rule.resource_id.clone(),
            ));
        }
        rules.push((resource, rule));
    }
    rules.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    Ok(rules)
}

fn province_land(world: &World) -> Result<BTreeMap<usize, ProvinceLand>, DepositGenerationError> {
    let map = world.map();
    let size = map
        .width
        .checked_mul(map.height)
        .ok_or(DepositGenerationError::InvalidMap)?;
    if map.width > i32::MAX as usize
        || map.height > i32::MAX as usize
        || map.terrain.len() != size
        || map.territory.len() != size
    {
        return Err(DepositGenerationError::InvalidMap);
    }
    let province_ids: HashSet<_> = world
        .provinces()
        .iter()
        .map(|province| province.id)
        .collect();
    let mut provinces = BTreeMap::<usize, ProvinceLand>::new();
    for (index, (area, &province_id)) in map.terrain.iter().zip(&map.territory).enumerate() {
        if !province_ids.contains(&province_id)
            || !area.elevation_val.is_finite()
            || !area.moisture_val.is_finite()
        {
            return Err(DepositGenerationError::InvalidMap);
        }
        let biome = area.determine_biome();
        if let Some(biome_index) = LAND_BIOMES.iter().position(|&land| land == biome) {
            let land = provinces.entry(province_id).or_default();
            land.biome_counts[biome_index] += 1;
            land.positions.push(Position {
                x: (index % map.width) as i32,
                y: (index / map.width) as i32,
            });
        }
    }
    Ok(provinces)
}

fn stream(seed: u64, province_id: usize, resource: &ResourceType, phase: u64) -> StdRng {
    // Fixed FNV-1a mixing avoids process-random hash state and content iteration order.
    let mut state = 0xcbf29ce484222325_u64;
    for byte in seed
        .to_le_bytes()
        .into_iter()
        .chain((province_id as u64).to_le_bytes())
        .chain(resource.as_str().bytes())
        .chain(phase.to_le_bytes())
    {
        state = (state ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    StdRng::seed_from_u64(state)
}
