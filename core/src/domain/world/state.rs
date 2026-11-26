use crate::domain::ids::SettlementId;
use std::collections::HashMap;
mod construction;
mod decrees;
mod diplomacy;
mod inspection;
mod map_commands;
mod nations;
mod resource_sites;
mod taxation;
mod trade;
pub use inspection::TileView;
pub(crate) use nations::NationOperationError;
pub use nations::{OwnershipChange, SubsidyTransfer};
pub use taxation::TaxCollection;

use super::settlement_registry::SettlementRegistry;
use super::territory::{TerritoryManager, Tile};
use crate::domain::buildings::catalog::BuildingCatalog;
use crate::domain::economy::{ResourceCatalog, ResourceType};
use crate::domain::ids::NationId;
use crate::domain::military::{UnitCatalog, UnitTypeId};
use crate::domain::nation::decree::DecreeId;
use crate::domain::nation::decree_catalog::DecreeCatalog;
use crate::domain::nation::diplomacy::Diplomacy;
use crate::domain::nation::registry::NationRegistry;
use crate::domain::settlement::{NeedRegistry, Settlement};
use crate::domain::trade::caravan::Caravan;
use crate::domain::world::{
    Position, coordinates::Point, map::Map, province::Province, province_graph::ProvinceGraph,
};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldError {
    SettlementIdExhausted,
    Project(crate::domain::settlement::projects::ProjectError),
    Building(crate::domain::settlement::BuildingCommandError),
    DuplicateSettlement(SettlementId),
    MissingSettlement(SettlementId),
    /// Importing existing structures requires spatial registration (a later stage).
    SettlementNotEmpty(SettlementId),
    SettlementResourceCatalogMismatch(SettlementId),
    UnknownPopulationMaterial(ResourceType),
    MissingProvince(usize),
    ProvinceOccupied {
        province_id: usize,
        settlement_id: SettlementId,
    },
    InvalidMapPosition(Position),
    InvalidTerrain(Position),
    NoCapitalSite(usize),
    CapitalOutsideProvince {
        position: Position,
        expected_province: usize,
        actual_province: usize,
    },
    CapitalOnOcean(Position),
    CapitalSiteProtected(Position),
    InvalidDepositResource(crate::domain::economy::ResourceType),
    DepositOnOcean(Position),
    DepositAlreadyExists(Position),
    RequiredDepositMissing {
        position: Position,
        resource: crate::domain::economy::ResourceType,
    },
    InconsistentOccupancy {
        position: Position,
        occupancy: super::territory::TileOccupancy,
    },
    SettlementOutsideProvince {
        province_id: usize,
        settlement_id: SettlementId,
        registered_settlement: Option<SettlementId>,
    },
    MissingNation(NationId),
    NationIdExhausted,
    /// The settlement already belongs to this nation.
    SettlementAlreadyOwned {
        settlement_id: SettlementId,
        nation_id: NationId,
    },
    /// The settlement does not belong to this nation.
    SettlementNotOwned {
        settlement_id: SettlementId,
        nation_id: NationId,
    },
    /// The national treasury or stock holds less than requested, or the
    /// settlement's gold counter cannot hold it.
    InsufficientNationalFunds(NationId),
    /// A unit type needs equipment that the world's resource catalogue lacks.
    UnitEquipmentUnknown(UnitTypeId),
    /// No decree with this ID in the world's decree catalogue.
    UnknownDecree(DecreeId),
    /// The decree cannot be enacted or revoked now (slot, state or duration).
    DecreeUnavailable {
        nation_id: NationId,
        decree: DecreeId,
    },
    /// The decree catalogue cannot be replaced while a nation has a decree
    /// that is pending, active or cooling down.
    DecreeCatalogInUse,
    /// The diplomacy rules cannot be replaced while a relation other than
    /// neutrality or a proposal is in progress.
    DiplomacyInProgress,
}

/// World layers are read-only outside the aggregate.
/// ```compile_fail
/// use simulation_core::world::World;
/// fn replace_layers(world: &mut World) {
///     world.map.territory.clear();
/// }
/// ```
pub struct World {
    completed_turn: u64,
    map: Map,
    provinces: Vec<Province>,
    pub caravans: Vec<Caravan>,
    territory: TerritoryManager,
    settlements: SettlementRegistry,
    province_settlements: HashMap<usize, SettlementId>,
    /// Building types agents may construct or upgrade to.
    building_catalog: BuildingCatalog,
    resource_catalog: Arc<ResourceCatalog>,
    /// Unit types and military rules (`units.toml`).
    unit_catalog: Arc<UnitCatalog>,
    /// Borders between provinces, rebuilt whenever a tile changes province
    /// or terrain.
    province_graph: ProvinceGraph,
    /// Decrees nations may enact and their common rules (`decrees.toml`).
    decree_catalog: Arc<DecreeCatalog>,
    /// Nations and the single record of which nation owns which settlement.
    nations: NationRegistry,
    /// Relations between nations, proposals and reputation (§8).
    diplomacy: Diplomacy,
}
impl World {
    pub(crate) fn completed_turn(&self) -> u64 {
        self.completed_turn
    }

    pub(crate) fn set_completed_turn(&mut self, turn: u64) {
        self.completed_turn = turn;
    }

    /// The province a settlement is registered in.
    pub(crate) fn settlement_province(&self, settlement_id: SettlementId) -> Option<usize> {
        self.province_settlements
            .iter()
            .find(|(_, id)| **id == settlement_id)
            .map(|(&province_id, _)| province_id)
    }

    /// Tiles of a province in row-major order.
    pub(crate) fn province_positions(&self, province_id: usize) -> Vec<Position> {
        let width = self.map.width;
        self.map
            .territory
            .iter()
            .enumerate()
            .filter(|(_, province)| **province == province_id)
            .filter_map(|(index, _)| {
                let position = Position {
                    x: i32::try_from(index % width).ok()?,
                    y: i32::try_from(index / width).ok()?,
                };
                self.map.tile_index(position).map(|_| position)
            })
            .collect()
    }

    pub fn building_catalog(&self) -> &BuildingCatalog {
        &self.building_catalog
    }

    pub fn resource_catalog(&self) -> &ResourceCatalog {
        &self.resource_catalog
    }

    /// Scenario setup: replaces the building types available to agents.
    pub fn set_building_catalog(&mut self, catalog: BuildingCatalog) {
        self.building_catalog = catalog;
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    /// Which provinces border each other, for army movement.
    pub fn province_graph(&self) -> &ProvinceGraph {
        &self.province_graph
    }

    pub(super) fn rebuild_province_graph(&mut self) {
        self.province_graph = ProvinceGraph::from_map(&self.map);
    }

    /// Unit types and military rules.
    pub fn unit_catalog(&self) -> &UnitCatalog {
        &self.unit_catalog
    }

    /// Replaces the bundled unit catalogue, e.g. with `units.toml` loaded at
    /// startup. Every equipment material must exist in this world's resource
    /// catalogue.
    pub fn set_unit_catalog(&mut self, catalog: Arc<UnitCatalog>) -> Result<(), WorldError> {
        if let Some(unit) = catalog.definitions().iter().find(|unit| {
            unit.equipment()
                .keys()
                .any(|resource| self.resource_catalog.get(resource).is_none())
        }) {
            return Err(WorldError::UnitEquipmentUnknown(unit.id().clone()));
        }
        self.unit_catalog = catalog;
        Ok(())
    }

    pub fn provinces(&self) -> &[Province] {
        &self.provinces
    }

    pub fn province(&self, id: usize) -> Result<&Province, WorldError> {
        self.provinces
            .iter()
            .find(|province| province.id == id)
            .ok_or(WorldError::MissingProvince(id))
    }

    /// Initializes and registers an empty settlement with an unused ID.
    /// A failed registration does not consume that ID.
    pub fn initialize_settlement(
        &mut self,
        province_id: usize,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Result<SettlementId, WorldError> {
        self.initialize_settlement_with_site(
            province_id,
            None,
            initial_capacity,
            initial_population,
            need_registry,
        )
    }

    pub fn initialize_settlement_at(
        &mut self,
        province_id: usize,
        capital: Position,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Result<SettlementId, WorldError> {
        self.initialize_settlement_with_site(
            province_id,
            Some(capital),
            initial_capacity,
            initial_population,
            need_registry,
        )
    }

    fn initialize_settlement_with_site(
        &mut self,
        province_id: usize,
        capital: Option<Position>,
        initial_capacity: u32,
        initial_population: u32,
        need_registry: Arc<NeedRegistry>,
    ) -> Result<SettlementId, WorldError> {
        let id = self
            .settlements
            .next_id()
            .ok_or(WorldError::SettlementIdExhausted)?;
        let settlement = Settlement::with_resource_catalog(
            id,
            initial_capacity,
            initial_population,
            need_registry,
            self.resource_catalog(),
        );
        self.register_settlement(province_id, capital, settlement, None)?;
        Ok(id)
    }

    pub fn settlement_registry(&self) -> &SettlementRegistry {
        &self.settlements
    }

    /// Internal split borrow for coordinated phases; never exposes mutable collections publicly.
    pub(crate) fn settlement_turn_parts<'a>(
        &'a mut self,
        settlement_id: SettlementId,
        finder: &'a mut crate::domain::world::pathfinding::path_finder::PathFinder,
    ) -> Option<(
        &'a mut Settlement,
        crate::domain::world::settlement_environment::MapSettlementEnvironment<'a>,
    )> {
        Some((
            self.settlements.get_mut(settlement_id)?,
            crate::domain::world::settlement_environment::MapSettlementEnvironment::new(
                &self.map,
                finder,
                &mut self.territory,
            ),
        ))
    }

    pub fn new(map: Map, provinces: Vec<Province>) -> World {
        Self::with_resource_catalog(
            map,
            provinces,
            Arc::new(ResourceCatalog::standard().clone()),
        )
    }

    pub fn with_resource_catalog(
        map: Map,
        provinces: Vec<Province>,
        resource_catalog: Arc<ResourceCatalog>,
    ) -> World {
        let province_graph = ProvinceGraph::from_map(&map);
        World {
            completed_turn: 0,
            map,
            provinces,
            caravans: Vec::new(),
            territory: TerritoryManager::default(),
            settlements: SettlementRegistry::default(),
            province_settlements: HashMap::new(),
            building_catalog: BuildingCatalog::default(),
            resource_catalog,
            unit_catalog: Arc::new(UnitCatalog::standard().clone()),
            province_graph,
            decree_catalog: Arc::new(DecreeCatalog::standard().clone()),
            nations: NationRegistry::default(),
            diplomacy: Diplomacy::default(),
        }
    }

    /// Registers an empty settlement on the first valid land tile in its province.
    /// Every settlement belongs to a nation: this one founds a new nation and
    /// becomes its capital (`add_settlement_to_nation` joins an existing one).
    pub fn add_settlement(
        &mut self,
        province_id: usize,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.register_settlement(province_id, None, settlement, None)
    }

    /// Registers an empty settlement at an explicit capital position; it founds
    /// a new nation like `add_settlement`.
    pub fn add_settlement_at(
        &mut self,
        province_id: usize,
        capital: Position,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.register_settlement(province_id, Some(capital), settlement, None)
    }

    /// Registers an empty settlement as a member of an existing nation
    /// (scenario setup).
    pub fn add_settlement_to_nation(
        &mut self,
        nation_id: NationId,
        province_id: usize,
        settlement: Settlement,
    ) -> Result<(), WorldError> {
        self.register_settlement(province_id, None, settlement, Some(nation_id))
    }

    /// `owner`: the nation to join, or None to found a new nation. All checks
    /// run before anything is registered, so an error changes nothing.
    fn register_settlement(
        &mut self,
        province_id: usize,
        capital: Option<Position>,
        mut settlement: Settlement,
        owner: Option<NationId>,
    ) -> Result<(), WorldError> {
        match owner {
            Some(nation_id) if self.nations.get(nation_id).is_none() => {
                return Err(WorldError::MissingNation(nation_id));
            }
            None if !self.nations.can_found() => return Err(WorldError::NationIdExhausted),
            _ => {}
        }
        let id = settlement.id();
        if self.settlements.get(id).is_some() {
            return Err(WorldError::DuplicateSettlement(id));
        }
        if let Some(settlement_id) = self.province_settlement(province_id)? {
            return Err(WorldError::ProvinceOccupied {
                province_id,
                settlement_id,
            });
        }
        if settlement.buildings().next().is_some() || !settlement.active_projects().is_empty() {
            return Err(WorldError::SettlementNotEmpty(id));
        }
        let world_materials = self.resource_catalog().materials();
        if settlement.storage_order().len() != world_materials.len()
            || world_materials
                .iter()
                .any(|material| !settlement.knows_material(material))
        {
            return Err(WorldError::SettlementResourceCatalogMismatch(id));
        }
        if let Some(resource) = settlement.first_unknown_population_material() {
            return Err(WorldError::UnknownPopulationMaterial(resource));
        }
        let capital = match capital {
            Some(position) => {
                self.validate_capital_site(province_id, position)?;
                position
            }
            None => self
                .province_positions(province_id)
                .into_iter()
                .find(|&position| self.validate_capital_site(province_id, position).is_ok())
                .ok_or(WorldError::NoCapitalSite(province_id))?,
        };
        settlement.assign_capital(capital);
        self.settlements
            .register(settlement)
            .map_err(WorldError::DuplicateSettlement)?;
        self.province_settlements.insert(province_id, id);
        match owner {
            Some(nation_id) => self.nations.add_member(id, nation_id),
            None => {
                self.nations.found(id).expect("checked by can_found");
            }
        }
        Ok(())
    }

    fn validate_capital_site(
        &self,
        province_id: usize,
        position: Position,
    ) -> Result<(), WorldError> {
        let index = self
            .map
            .tile_index(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        let actual_province = self.map.territory[index];
        if actual_province != province_id {
            return Err(WorldError::CapitalOutsideProvince {
                position,
                expected_province: province_id,
                actual_province,
            });
        }
        let terrain = &self.map.terrain[index];
        if !terrain.elevation_val.is_finite() || !terrain.moisture_val.is_finite() {
            return Err(WorldError::InvalidTerrain(position));
        }
        if terrain.determine_elevation() == super::terrain::elevation::Elevation::Ocean {
            return Err(WorldError::CapitalOnOcean(position));
        }
        Ok(())
    }

    fn is_capital_site(&self, index: usize, position: Position) -> bool {
        self.province_settlements
            .get(&self.map.territory[index])
            .and_then(|&id| self.settlement(id))
            .and_then(Settlement::capital)
            == Some(position)
    }

    pub fn settlement(&self, id: SettlementId) -> Option<&Settlement> {
        self.settlements.get(id)
    }

    pub(crate) fn settlement_mut(
        &mut self,
        id: SettlementId,
    ) -> Result<&mut Settlement, WorldError> {
        self.settlements
            .get_mut(id)
            .ok_or(WorldError::MissingSettlement(id))
    }

    pub(crate) fn settlement_ids(&self) -> Vec<SettlementId> {
        self.settlements.iter().map(Settlement::id).collect()
    }

    pub fn settlements(&self) -> impl Iterator<Item = &Settlement> {
        self.settlements.iter()
    }

    /// Shared access only: mutations will be coordinated with settlements.
    pub fn territory(&self) -> &TerritoryManager {
        &self.territory
    }

    /// Returns the settlement ID in a province, or None for an empty province.
    pub fn province_settlement(
        &self,
        province_id: usize,
    ) -> Result<Option<SettlementId>, WorldError> {
        self.province(province_id)?;
        Ok(self.province_settlements.get(&province_id).copied())
    }

    /// Resolves a map position to an existing province ID, not a vector offset.
    pub fn province_at(&self, position: Position) -> Result<usize, WorldError> {
        let id = self
            .map
            .province_at(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        self.province_settlement(id)?;
        Ok(id)
    }

    pub fn settlement_at(&self, position: Position) -> Result<Option<&Settlement>, WorldError> {
        let province_id = self.province_at(position)?;
        Ok(self
            .province_settlements
            .get(&province_id)
            .and_then(|id| self.settlements.get(*id)))
    }

    /// Unlike the standalone registry, this read rejects invalid map positions.
    pub fn tile_at(&self, position: Position) -> Result<Tile, WorldError> {
        self.province_at(position)?;
        Ok(self.territory.tile_at(position))
    }

    /// Checks that the position belongs to the settlement's province.
    pub fn validate_settlement_position(
        &self,
        settlement_id: SettlementId,
        position: Position,
    ) -> Result<(), WorldError> {
        if self.settlements.get(settlement_id).is_none() {
            return Err(WorldError::MissingSettlement(settlement_id));
        }
        let province_id = self.province_at(position)?;
        let registered_settlement = self.province_settlement(province_id)?;
        if registered_settlement != Some(settlement_id) {
            return Err(WorldError::SettlementOutsideProvince {
                province_id,
                settlement_id,
                registered_settlement,
            });
        }
        Ok(())
    }
    fn caravan_travel(&mut self) {
        for caravan in &mut self.caravans {
            caravan.travel();
        }
    }
    pub fn centroid(&self, province_id: usize) -> Result<Point, WorldError> {
        Ok(self.province(province_id)?.centroid())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::world::settlement_environment::{
        MapConstructionSpace, MapSettlementEnvironment,
    };

    use super::*;
    use crate::domain::buildings::definition::BuildingDefinition;
    use crate::domain::ids::ProjectId;
    use crate::domain::ids::SettlementId;
    use crate::domain::settlement::projects::ProjectCost;
    use crate::domain::world::terrain::area::Area;
    use crate::domain::world::territory::TerritoryManager;
    use crate::domain::world::territory::TileOccupancy;
    use std::sync::Arc;

    fn world() -> World {
        // IDs deliberately differ from province vector indices.
        World::new(
            Map::new(
                3,
                2,
                vec![Area::new(0.5, 0.5); 6],
                vec![10, 20, 30, 10, 20, 30],
            ),
            vec![Province::new(30), Province::new(10), Province::new(20)],
        )
    }

    fn settlement(id: u32) -> Settlement {
        Settlement::new(SettlementId(id), 100, 0, Arc::default())
    }

    #[test]
    fn world_starts_with_empty_provinces_and_tiles() {
        let world = world();
        assert_eq!(world.settlements().count(), 0);
        assert_eq!(world.province_settlement(10), Ok(None));
        assert!(
            world
                .settlement_at(Position { x: 0, y: 0 })
                .unwrap()
                .is_none()
        );
        assert_eq!(
            world.tile_at(Position { x: 0, y: 0 }).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    #[test]
    fn registration_and_deposits_use_the_world_catalog() {
        let wood = ResourceCatalog::standard().resolve("wood").unwrap();
        let resources = Arc::new(
            ResourceCatalog::from_toml(
                r#"
                [[resources]]
                id = "salt"
                name = "Salt"
                tier = 1
                category = "raw_food"
                "#,
            )
            .unwrap(),
        );
        let salt = resources.resolve("salt").unwrap();
        let position = Position { x: 0, y: 0 };
        let mut world = World::with_resource_catalog(
            Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![10]),
            vec![Province::new(10)],
            resources,
        );
        assert_eq!(
            world.add_settlement(10, settlement(1)),
            Err(WorldError::SettlementResourceCatalogMismatch(SettlementId(
                1
            )))
        );
        assert_eq!(world.province_settlement(10), Ok(None));
        let needs = Arc::new(NeedRegistry::new(vec![
            crate::domain::settlement::NeedTier {
                min_population: 100,
                needs_per_capita: HashMap::from([(wood.clone(), 1)]),
            },
        ]));
        assert_eq!(
            world.initialize_settlement(10, 100, 1, needs),
            Err(WorldError::UnknownPopulationMaterial(wood.clone()))
        );
        assert_eq!(world.province_settlement(10), Ok(None));
        let id = world
            .initialize_settlement(10, 100, 1, Arc::default())
            .unwrap();
        assert_eq!(
            world.settlement(id).unwrap().storage_order(),
            std::slice::from_ref(&salt)
        );
        assert_eq!(
            world.place_deposit(position, wood.clone()),
            Err(WorldError::InvalidDepositResource(wood))
        );
        world.place_deposit(position, salt.clone()).unwrap();
        assert_eq!(world.accessible_deposits(id), Ok(vec![(position, salt)]));
    }

    #[test]
    fn settlements_are_registered_in_distinct_provinces() {
        let mut world = world();
        world.add_settlement(10, settlement(1)).unwrap();
        world.add_settlement(30, settlement(2)).unwrap();

        for (x, province, settlement_id) in [(0, 10, 1), (2, 30, 2)] {
            for y in 0..2 {
                let position = Position { x, y };
                assert_eq!(world.province_at(position), Ok(province));
                assert_eq!(
                    world.settlement_at(position).unwrap().unwrap().id(),
                    SettlementId(settlement_id)
                );
                assert_eq!(
                    world.validate_settlement_position(SettlementId(settlement_id), position),
                    Ok(())
                );
            }
        }
        assert_eq!(world.province_settlement(20), Ok(None));
        assert!(
            world
                .settlement_at(Position { x: 1, y: 0 })
                .unwrap()
                .is_none()
        );
        assert_eq!(world.settlements().count(), 2);
        assert_eq!(world.province_settlements.len(), 2);
    }

    #[test]
    fn occupied_and_missing_provinces_reject_registration_without_changes() {
        let mut world = world();
        world.add_settlement(10, settlement(1)).unwrap();
        let before = world.province_settlements.clone();
        assert_eq!(
            world.add_settlement(10, settlement(2)),
            Err(WorldError::ProvinceOccupied {
                province_id: 10,
                settlement_id: SettlementId(1)
            })
        );
        assert_eq!(
            world.add_settlement(99, settlement(3)),
            Err(WorldError::MissingProvince(99))
        );
        assert_eq!(world.province_settlements, before);
        assert_eq!(world.settlements().count(), 1);
        assert!(world.settlement(SettlementId(2)).is_none());
        assert!(world.settlement(SettlementId(3)).is_none());
    }

    #[test]
    fn settlement_cannot_be_registered_in_a_second_province() {
        let mut world = world();
        world
            .add_settlement(10, Settlement::new(SettlementId(1), 100, 7, Arc::default()))
            .unwrap();
        for province_id in [10, 20] {
            assert_eq!(
                world.add_settlement(province_id, settlement(1)),
                Err(WorldError::DuplicateSettlement(SettlementId(1)))
            );
        }
        assert_eq!(world.settlement(SettlementId(1)).unwrap().population(), 7);
        assert_eq!(world.settlements().count(), 1);
        assert_eq!(world.province_settlement(10), Ok(Some(SettlementId(1))));
        assert_eq!(world.province_settlement(20), Ok(None));
        assert_eq!(world.province_settlements.len(), 1);
        assert!(world.settlement(SettlementId(99)).is_none());
    }

    #[test]
    fn preexisting_projects_cannot_bypass_spatial_registration() {
        let mut territory = TerritoryManager::default();
        let mut world = world();
        let mut settlement = settlement(1);
        let definition = Arc::new(
            BuildingDefinition::new("house", "House", 1, 0, vec![], ProjectCost::default())
                .unwrap(),
        );
        settlement
            .queue_construction(
                ProjectId(1),
                Position { x: 0, y: 0 },
                definition,
                ProjectCost::default(),
                &mut MapConstructionSpace::new(&world.map, &mut territory),
            )
            .unwrap();
        assert_eq!(
            world.add_settlement(10, settlement),
            Err(WorldError::SettlementNotEmpty(SettlementId(1)))
        );
        assert_eq!(world.settlements().count(), 0);
        assert!(world.province_settlements.is_empty());
    }

    #[test]
    fn preexisting_buildings_cannot_bypass_spatial_registration() {
        let mut territory = TerritoryManager::default();
        use crate::domain::buildings::Building;
        use crate::domain::ids::BuildingId;
        use crate::domain::world::pathfinding::path_finder::PathFinder;

        let mut world = world();
        let mut settlement = settlement(1);
        let definition = Arc::new(
            BuildingDefinition::new("house", "House", 1, 0, vec![], ProjectCost::default())
                .unwrap(),
        );
        let position = Position { x: 0, y: 0 };
        let building = Building::new(BuildingId(1), position, definition);
        settlement
            .add_building(
                building,
                &mut MapSettlementEnvironment::new(
                    &world.map,
                    &mut PathFinder::new(3, 2),
                    &mut territory,
                ),
            )
            .unwrap();
        assert_eq!(
            world.add_settlement(10, settlement),
            Err(WorldError::SettlementNotEmpty(SettlementId(1)))
        );
        assert!(world.settlement(SettlementId(1)).is_none());
        assert_eq!(world.province_settlement(10), Ok(None));
        assert_eq!(
            world.tile_at(position).unwrap().occupancy(),
            TileOccupancy::Empty
        );
    }

    #[test]
    fn position_validation_rejects_empty_and_other_settlements_provinces() {
        let mut world = world();
        world.add_settlement(30, settlement(1)).unwrap();
        world.add_settlement(10, settlement(2)).unwrap();
        for (position, province_id, registered_settlement) in [
            (Position { x: 0, y: 0 }, 10, Some(SettlementId(2))),
            (Position { x: 1, y: 0 }, 20, None),
        ] {
            assert_eq!(
                world.validate_settlement_position(SettlementId(1), position),
                Err(WorldError::SettlementOutsideProvince {
                    province_id,
                    settlement_id: SettlementId(1),
                    registered_settlement
                })
            );
        }
        assert_eq!(
            world.validate_settlement_position(SettlementId(99), Position { x: 0, y: 0 }),
            Err(WorldError::MissingSettlement(SettlementId(99)))
        );
    }

    #[test]
    fn invalid_positions_are_not_reported_as_empty_tiles() {
        let world = world();
        for position in [
            Position { x: -1, y: 0 },
            Position { x: 0, y: -1 },
            Position { x: 3, y: 0 },
            Position { x: 0, y: 2 },
            Position {
                x: i32::MAX,
                y: i32::MAX,
            },
        ] {
            assert_eq!(
                world.tile_at(position),
                Err(WorldError::InvalidMapPosition(position))
            );
            assert_eq!(
                world.province_at(position),
                Err(WorldError::InvalidMapPosition(position))
            );
            assert!(
                matches!(world.settlement_at(position), Err(WorldError::InvalidMapPosition(p)) if p == position)
            );
        }
    }

    #[test]
    fn malformed_map_layers_and_unknown_provinces_return_errors() {
        let mut world = world();
        let position = Position { x: 2, y: 1 };
        world.map.territory[5] = 99;
        assert_eq!(
            world.tile_at(position),
            Err(WorldError::MissingProvince(99))
        );
        world.map.territory.pop();
        assert_eq!(
            world.tile_at(position),
            Err(WorldError::InvalidMapPosition(position))
        );
        world.map.terrain.clear();
        assert_eq!(
            world.tile_at(Position { x: 0, y: 0 }),
            Err(WorldError::InvalidMapPosition(Position { x: 0, y: 0 }))
        );
    }
}
