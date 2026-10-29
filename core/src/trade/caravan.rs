use crate::ids::SettlementId;
use crate::trade::book::TradeBook;
use crate::world::pathfinding::{a_star::AStar, path_finder::PathFinder};
use crate::world::{Map, Point};
const MOVE_SPEED: usize = 10;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaravanState {
    TravelTo,
    Trading,
    TravelBack,
    Back,
}

pub struct Caravan {
    pub sender_id: SettlementId,
    pub receiver_id: SettlementId,
    current_position: Point,
    current_position_idx: usize,
    caravan_state: CaravanState,
    path: Vec<Point>,
    book: TradeBook,
}

impl Caravan {
    /// Build a route between explicit map points; return None if no route exists.
    /// Participant IDs are carried as identity data and are not used as map indices.
    pub fn new(
        map: &Map,
        path_finder: &mut PathFinder,
        sender_id: SettlementId,
        receiver_id: SettlementId,
        start: Point,
        end: Point,
        book: TradeBook,
    ) -> Option<Caravan> {
        let current_position: Point = start;
        let current_position_idx: usize = 0;
        let caravan_state: CaravanState = CaravanState::TravelTo;
        let path: Vec<Point> = AStar::a_star(map, path_finder, (start.0, start.1), (end.0, end.1))?;
        Some(Caravan {
            sender_id,
            receiver_id,
            current_position,
            current_position_idx,
            caravan_state,
            path,
            book,
        })
    }
    pub fn travel(&mut self) {
        match self.caravan_state {
            CaravanState::TravelTo => self.travel_to(),
            CaravanState::TravelBack => self.travel_back(),
            _ => {}
        }
    }
    pub fn trade(&mut self, book: &mut TradeBook) {
        if self.caravan_state == CaravanState::Trading {
            book.trade(&mut self.book);
            self.caravan_state = CaravanState::TravelBack;
        }
    }
    fn travel_to(&mut self) {
        if self.current_position_idx + MOVE_SPEED < self.path.len() - 1 {
            self.current_position_idx += MOVE_SPEED;
            self.current_position = self.path[self.current_position_idx];
        } else {
            self.current_position_idx = self.path.len() - 1;
            self.current_position = self.path[self.current_position_idx];
            self.caravan_state = CaravanState::Trading;
        }
    }
    fn travel_back(&mut self) {
        if 0 < self.current_position_idx - MOVE_SPEED {
            self.current_position_idx -= MOVE_SPEED;
            self.current_position = self.path[self.current_position_idx];
        } else {
            self.current_position_idx = 0;
            self.current_position = self.path[self.current_position_idx];
            self.caravan_state = CaravanState::Back;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::area::Area;

    #[test]
    fn construction_uses_explicit_endpoints_without_a_world_or_province_lookup() {
        let map = Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![0; 3]);
        let mut finder = PathFinder::new(3, 1);
        let caravan = Caravan::new(
            &map,
            &mut finder,
            SettlementId(100),
            SettlementId(200),
            Point(0, 0),
            Point(2, 0),
            TradeBook::default(),
        )
        .unwrap();
        assert_eq!(caravan.sender_id, SettlementId(100));
        assert_eq!(caravan.receiver_id, SettlementId(200));
        assert_eq!(caravan.current_position, Point(0, 0));
        assert_eq!(caravan.path, vec![Point(0, 0), Point(1, 0), Point(2, 0)]);
        assert_eq!(caravan.caravan_state, CaravanState::TravelTo);
    }

    #[test]
    fn construction_returns_none_for_unreachable_or_invalid_endpoints() {
        let mut map = Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![0; 3]);
        map.terrain[1].elevation_val = 0.0;
        let mut finder = PathFinder::new(3, 1);
        for end in [Point(2, 0), Point(1, 0), Point(3, 0)] {
            assert!(
                Caravan::new(
                    &map,
                    &mut finder,
                    SettlementId(100),
                    SettlementId(200),
                    Point(0, 0),
                    end,
                    TradeBook::default(),
                )
                .is_none()
            );
        }
    }
}
