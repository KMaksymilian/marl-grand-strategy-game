use std::collections::{HashMap, HashSet};
use crate::domain::world_data::point::Point;

//pub type ProvinceId = u32;
//pub type SettlementId = u32;

#[derive(Debug, Default)]
pub struct TerritoryManager {
    pub tile_to_province: HashMap<Point, u32>,
    pub tile_to_influence: HashMap<Point, u32>,


    pub province_tiles: HashMap<u32, Vec<Point>>,
    pub settlement_influence: HashMap<u32, HashSet<Point>>,


    pub settlement_to_province: HashMap<u32, u32>,
    pub province_settlements: HashMap<u32, HashSet<u32>>,
}

impl TerritoryManager {


    pub fn new() -> Self {
        Self::default()
    }


    /// Register new settlement
    /// Return error if settlement exist in province
    pub fn register_settlement(
        &mut self,
        settlement_id: u32,
        center: Point
    ) -> Result<(), &'static str> {
        // 1. Province is determined by center placement
        let province_id = match self.tile_to_province.get(&center) {
            Some(&id) => id,
            None => return Err("No province for this tile."),
        };

        // 2. Link settlement with province
        self.settlement_to_province.insert(settlement_id, province_id);

        // 3. Register settlement in province
        self.province_settlements
            .entry(province_id)
            .or_default()
            .insert(settlement_id);

        // Optional
        self.claim_influence(center, settlement_id);

        Ok(())
    }

    /// Remove Settlement from map
    pub fn destroy_settlement(&mut self, settlement_id: u32) {
        if let Some(province_id) = self.settlement_to_province.remove(&settlement_id) {
            if let Some(settlements) = self.province_settlements.get_mut(&province_id) {
                settlements.remove(&settlement_id);
            }
        }

        let tiles_to_clear: Vec<Point> = self.settlement_influence
            .get(&settlement_id)
            .map(|tiles| tiles.iter().copied().collect())
            .unwrap_or_default();

        for pos in tiles_to_clear {
            self.remove_influence(&pos);
        }
    }

    pub fn assign_to_province(&mut self, pos: Point, province_id: u32) {
        self.tile_to_province.insert(pos, province_id);
        self.province_tiles
            .entry(province_id)
            .or_default()
            .push(pos);
    }


    /// Assigning tile to Settlement influence zone
    pub fn claim_influence(&mut self, pos: Point, settlement_id: u32) {
        if let Some(&old_settlement) = self.tile_to_influence.get(&pos) {
            if old_settlement != settlement_id {
                if let Some(tiles) = self.settlement_influence.get_mut(&old_settlement) {
                    tiles.remove(&pos);
                }
            } else {
                return;
            }
        }

        self.tile_to_influence.insert(pos, settlement_id);

        self.settlement_influence
            .entry(settlement_id)
            .or_default()
            .insert(pos);
    }

    /// Remove Settlement influence from tile
    pub fn remove_influence(&mut self, pos: &Point) {
        if let Some(settlement_id) = self.tile_to_influence.remove(pos) {
            if let Some(tiles) = self.settlement_influence.get_mut(&settlement_id) {
                tiles.remove(pos);

                if tiles.is_empty() {
                    self.settlement_influence.remove(&settlement_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a dummy point. 
    // Adjust this to match your actual Point struct constructor.
    fn create_point(x: usize, y: usize) -> Point {
        Point( x, y )
    }

    #[test]
    fn test_assign_to_province_adds_tile_to_correct_maps() {
        // Arrange: Create a new TerritoryManager, define a tile and a province ID
        let mut manager = TerritoryManager::new();
        let tile = create_point(1, 1);
        let province_id = 10;

        // Act: Assign the tile to the province
        manager.assign_to_province(tile, province_id);

        // Assert: Verify the tile is mapped to the province in both HashMaps
        assert_eq!(manager.tile_to_province.get(&tile), Some(&province_id));
        assert!(manager.province_tiles.get(&province_id).unwrap().contains(&tile));
    }

    #[test]
    fn test_register_settlement_succeeds_when_tile_has_province() {
        // Arrange: Setup manager with a pre-assigned province tile
        let mut manager = TerritoryManager::new();
        let center_tile = create_point(5, 5);
        let province_id = 10;
        let settlement_id = 100;

        manager.assign_to_province(center_tile, province_id);

        // Act: Register the settlement on that tile
        let result = manager.register_settlement(settlement_id, center_tile);

        // Assert: Verify successful registration and proper data linkages
        assert!(result.is_ok());

        // Check settlement <-> province links
        assert_eq!(manager.settlement_to_province.get(&settlement_id), Some(&province_id));
        assert!(manager.province_settlements.get(&province_id).unwrap().contains(&settlement_id));

        // Check automatic influence claim over the center tile
        assert_eq!(manager.tile_to_influence.get(&center_tile), Some(&settlement_id));
    }

    #[test]
    fn test_register_settlement_fails_when_tile_has_no_province() {
        // Arrange: Setup manager WITHOUT assigning the tile to any province
        let mut manager = TerritoryManager::new();
        let wild_tile = create_point(10, 10);
        let settlement_id = 100;

        // Act: Attempt to register the settlement in the wilderness
        let result = manager.register_settlement(settlement_id, wild_tile);

        // Assert: Verify it returns the expected error and no data is mutated
        assert_eq!(result, Err("No province for this tile."));
        assert!(manager.settlement_to_province.is_empty());
    }

    #[test]
    fn test_claim_influence_steals_tile_from_previous_owner() {
        // Arrange: Setup manager with two settlements and one contested tile
        let mut manager = TerritoryManager::new();
        let contested_tile = create_point(2, 2);
        let old_owner_id = 100;
        let new_owner_id = 200;

        manager.claim_influence(contested_tile, old_owner_id);

        // Act: New owner claims the already occupied tile
        manager.claim_influence(contested_tile, new_owner_id);

        // Assert: Verify influence is transferred and old owner loses the tile
        assert_eq!(manager.tile_to_influence.get(&contested_tile), Some(&new_owner_id));

        // New owner has the tile
        assert!(manager.settlement_influence.get(&new_owner_id).unwrap().contains(&contested_tile));

        // Old owner no longer has the tile in their influence set
        assert!(!manager.settlement_influence.get(&old_owner_id).unwrap().contains(&contested_tile));
    }

    #[test]
    fn test_remove_influence_cleans_up_empty_sets() {
        // Arrange: Setup manager with a settlement having exactly one tile
        let mut manager = TerritoryManager::new();
        let tile = create_point(3, 3);
        let settlement_id = 100;

        manager.claim_influence(tile, settlement_id);

        // Act: Remove influence from that single tile
        manager.remove_influence(&tile);

        // Assert: Verify the tile is freed and the empty HashSet is completely removed
        assert!(manager.tile_to_influence.get(&tile).is_none());
        assert!(manager.settlement_influence.get(&settlement_id).is_none());
    }

    #[test]
    fn test_destroy_settlement_removes_all_traces() {
        // Arrange: Setup manager with a fully registered settlement owning multiple tiles
        let mut manager = TerritoryManager::new();
        let center = create_point(0, 0);
        let expansion_tile = create_point(0, 1);
        let province_id = 10;
        let settlement_id = 100;

        manager.assign_to_province(center, province_id);
        let _ = manager.register_settlement(settlement_id, center);
        manager.claim_influence(expansion_tile, settlement_id);

        // Act: Destroy the settlement
        manager.destroy_settlement(settlement_id);

        // Assert: Verify all links to provinces and tiles are completely erased
        assert!(manager.settlement_to_province.get(&settlement_id).is_none());
        assert!(!manager.province_settlements.get(&province_id).unwrap().contains(&settlement_id));

        // Influence maps should be cleared for both tiles
        assert!(manager.tile_to_influence.get(&center).is_none());
        assert!(manager.tile_to_influence.get(&expansion_tile).is_none());
        assert!(manager.settlement_influence.get(&settlement_id).is_none());
    }
}