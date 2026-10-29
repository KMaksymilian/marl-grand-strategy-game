//! Combined read-only tile data for world inspection, not an agent's knowledge.
use super::{World, WorldError};
use crate::buildings::Building;
use crate::settlement::{Settlement, projects::Project};
use crate::world::Position;
use crate::world::terrain::area::Area;
use crate::world::territory::{Tile, TileOccupancy};

/// Borrows authoritative world data; callers cannot mutate its referenced objects.
/// Deposits, units and visibility filtering are not implemented yet.
pub struct TileView<'a> {
    pub position: Position,
    pub terrain: &'a Area,
    pub province_id: usize,
    /// Settlement registered in this province, if any.
    pub settlement: Option<&'a Settlement>,
    pub tile: Tile,
    pub building: Option<&'a Building>,
    /// New construction only; upgrades are in the settlement's active projects.
    pub project: Option<&'a Project>,
}

impl World {
    pub fn inspect_tile(&self, position: Position) -> Result<TileView<'_>, WorldError> {
        let index = self
            .map
            .tile_index(position)
            .ok_or(WorldError::InvalidMapPosition(position))?;
        let province_id = self.province_at(position)?;
        let settlement = self
            .province_settlements
            .get(&province_id)
            .and_then(|id| self.settlement(*id));
        let tile = self.territory.tile_at(position);
        let (building, project) = match tile.occupancy() {
            TileOccupancy::Empty => (None, None),
            TileOccupancy::Construction(reference) => (
                None,
                self.settlement(reference.settlement_id)
                    .and_then(|settlement| {
                        settlement
                            .active_projects()
                            .iter()
                            .find(|project| project.id() == reference.project_id)
                    }),
            ),
            TileOccupancy::Building(reference) => (
                self.settlement(reference.settlement_id)
                    .and_then(|settlement| {
                        settlement
                            .buildings()
                            .find(|building| building.instance_id() == reference.building_id)
                    }),
                None,
            ),
        };
        Ok(TileView {
            position,
            terrain: &self.map.terrain[index],
            province_id,
            settlement,
            tile,
            building,
            project,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildings::definition::BuildingDefinition;
    use crate::ids::ProjectId;
    use crate::ids::SettlementId;
    use crate::settlement::projects::ProjectCost;
    use crate::world::{Map, Province, pathfinding::path_finder::PathFinder};
    use std::sync::Arc;

    fn world() -> World {
        let mut world = World::new(
            Map::new(
                2,
                1,
                vec![Area::new(0.5, 0.6), Area::new(0.1, 0.2)],
                vec![10, 20],
            ),
            vec![Province::new(10), Province::new(20)],
        );
        world
            .add_settlement(10, Settlement::new(SettlementId(1), 100, 0, Arc::default()))
            .unwrap();
        world
    }

    fn definition() -> Arc<BuildingDefinition> {
        Arc::new(BuildingDefinition {
            id: "house".into(),
            name: "House".into(),
            tier: 1,
            max_workers: 0,
            behaviors: vec![],
            construction_cost: ProjectCost::default(),
        })
    }

    #[test]
    fn inspection_combines_terrain_province_settlement_and_empty_occupancy() {
        let world = world();
        let position = Position { x: 0, y: 0 };
        let view = world.inspect_tile(position).unwrap();
        assert_eq!(view.position, position);
        assert!(std::ptr::eq(view.terrain, &world.map.terrain[0]));
        assert_eq!(view.province_id, 10);
        assert_eq!(view.settlement.unwrap().id(), SettlementId(1));
        assert_eq!(view.tile.occupancy(), TileOccupancy::Empty);
        assert!(view.project.is_none());
        assert!(view.building.is_none());
    }

    #[test]
    fn uninhabited_ocean_tiles_can_be_inspected() {
        let world = world();
        let view = world.inspect_tile(Position { x: 1, y: 0 }).unwrap();
        assert_eq!(view.province_id, 20);
        assert_eq!(view.terrain.elevation_val, 0.1);
        assert!(view.settlement.is_none());
        assert_eq!(view.tile.occupancy(), TileOccupancy::Empty);
    }

    #[test]
    fn inspection_resolves_project_then_building_without_copying_objects() {
        let mut world = world();
        let position = Position { x: 0, y: 0 };
        world
            .queue_construction(
                SettlementId(1),
                ProjectId(7),
                position,
                definition(),
                ProjectCost::default(),
            )
            .unwrap();
        let view = world.inspect_tile(position).unwrap();
        assert!(std::ptr::eq(
            view.project.unwrap(),
            &world.settlement(SettlementId(1)).unwrap().active_projects()[0]
        ));
        assert!(view.building.is_none());

        world
            .process_settlement_turn(SettlementId(1), &mut PathFinder::new(2, 1))
            .unwrap();
        let view = world.inspect_tile(position).unwrap();
        assert!(view.project.is_none());
        assert!(std::ptr::eq(
            view.building.unwrap(),
            world
                .settlement(SettlementId(1))
                .unwrap()
                .buildings()
                .next()
                .unwrap()
        ));
        assert_eq!(view.building.unwrap().position(), position);
    }

    #[test]
    fn canceled_construction_disappears_from_inspection() {
        let mut world = world();
        let position = Position { x: 0, y: 0 };
        world
            .queue_construction(
                SettlementId(1),
                ProjectId(7),
                position,
                definition(),
                ProjectCost::default(),
            )
            .unwrap();
        world.cancel_project(SettlementId(1), ProjectId(7)).unwrap();
        let view = world.inspect_tile(position).unwrap();
        assert!(view.project.is_none());
        assert!(view.building.is_none());
        assert_eq!(view.tile.occupancy(), TileOccupancy::Empty);
    }

    #[test]
    fn invalid_positions_and_unknown_provinces_are_rejected() {
        let mut world = world();
        for position in [Position { x: -1, y: 0 }, Position { x: 2, y: 0 }] {
            assert!(
                matches!(world.inspect_tile(position), Err(WorldError::InvalidMapPosition(p)) if p == position)
            );
        }
        world.map.territory[0] = 99;
        assert!(matches!(
            world.inspect_tile(Position { x: 0, y: 0 }),
            Err(WorldError::MissingProvince(99))
        ));
        world.map.terrain.clear();
        assert!(matches!(
            world.inspect_tile(Position { x: 0, y: 0 }),
            Err(WorldError::InvalidMapPosition(_))
        ));
    }
}
