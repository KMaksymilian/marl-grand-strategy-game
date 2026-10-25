use crate::trade::book::TradeBook;
use crate::trade::execution::TradeExecution;
use crate::world::pathfinding::{a_star::AStar, path_finder::PathFinder};
use crate::world::{Map, Point};
const MOVE_SPEED: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaravanState {
    TravelTo,
    Trading,
    TravelBack,
    Back,
}

/// How a caravan's journey ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaravanOutcome {
    /// Reached the receiver and traded (possibly zero executions).
    Traded,
    /// No settlement with the receiver ID existed when the caravan arrived or left.
    ReceiverMissing,
    /// No route between sender and receiver; the caravan never left.
    NoRoute,
}

/// Funded book leaving a settlement, before a route is assigned.
#[derive(Debug)]
pub struct CaravanCargo {
    pub sender_id: u32,
    pub receiver_id: u32,
    pub book: TradeBook,
}

/// Book carried back to the sender, with the trades made on the journey.
#[derive(Debug)]
pub struct CaravanReturn {
    pub receiver_id: u32,
    pub outcome: CaravanOutcome,
    pub book: TradeBook,
    /// Executions seen from the caravan (sender) side.
    pub trades: Vec<TradeExecution>,
}

impl CaravanReturn {
    /// Cargo that never travelled, returned unchanged.
    pub fn undelivered(cargo: CaravanCargo, outcome: CaravanOutcome) -> Self {
        Self {
            receiver_id: cargo.receiver_id,
            outcome,
            book: cargo.book,
            trades: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Caravan {
    pub sender_id: usize,
    pub receiver_id: usize,
    current_position: Point,
    current_position_idx: usize,
    caravan_state: CaravanState,
    path: Vec<Point>,
    book: TradeBook,
    trades: Vec<TradeExecution>,
    receiver_missing: bool,
}

impl Caravan {
    /// Build a route between explicit map points; return None if no route exists.
    /// Participant IDs are carried as identity data and are not used as map indices.
    pub fn new(
        map: &Map,
        path_finder: &mut PathFinder,
        sender_id: usize,
        receiver_id: usize,
        start: Point,
        end: Point,
        book: TradeBook,
    ) -> Option<Caravan> {
        let path: Vec<Point> = AStar::a_star(map, path_finder, (start.0, start.1), (end.0, end.1))?;
        Some(Self::with_path(sender_id, receiver_id, path, book))
    }

    /// Like `new`, but hands the cargo back when no route exists so nothing is lost.
    pub fn from_cargo(
        map: &Map,
        path_finder: &mut PathFinder,
        cargo: CaravanCargo,
        start: Point,
        end: Point,
    ) -> Result<Caravan, CaravanCargo> {
        match AStar::a_star(map, path_finder, (start.0, start.1), (end.0, end.1)) {
            Some(path) if !path.is_empty() => Ok(Self::with_path(
                cargo.sender_id as usize,
                cargo.receiver_id as usize,
                path,
                cargo.book,
            )),
            _ => Err(cargo),
        }
    }

    fn with_path(sender_id: usize, receiver_id: usize, path: Vec<Point>, book: TradeBook) -> Self {
        Caravan {
            sender_id,
            receiver_id,
            current_position: path[0],
            current_position_idx: 0,
            caravan_state: CaravanState::TravelTo,
            path,
            book,
            trades: Vec::new(),
            receiver_missing: false,
        }
    }

    pub fn state(&self) -> CaravanState {
        self.caravan_state
    }

    pub fn position(&self) -> Point {
        self.current_position
    }

    pub fn book(&self) -> &TradeBook {
        &self.book
    }

    /// Trades made so far, seen from the caravan side.
    pub fn trades(&self) -> &[TradeExecution] {
        &self.trades
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
            let executed = book.trade(&mut self.book);
            self.finish_trading(executed);
        }
    }

    pub(crate) fn book_mut(&mut self) -> &mut TradeBook {
        &mut self.book
    }

    /// Records executions seen from the receiver's book and starts the way home.
    pub(crate) fn finish_trading(&mut self, receiver_side: Vec<TradeExecution>) {
        if self.caravan_state == CaravanState::Trading {
            self.trades
                .extend(receiver_side.into_iter().map(TradeExecution::mirrored));
            self.caravan_state = CaravanState::TravelBack;
        }
    }

    /// Turns back without trading because the receiver does not exist.
    pub(crate) fn abort_trading(&mut self) {
        if self.caravan_state == CaravanState::Trading {
            self.receiver_missing = true;
            self.caravan_state = CaravanState::TravelBack;
        }
    }

    pub fn into_return(self) -> CaravanReturn {
        CaravanReturn {
            receiver_id: self.receiver_id as u32,
            outcome: if self.receiver_missing {
                CaravanOutcome::ReceiverMissing
            } else {
                CaravanOutcome::Traded
            },
            book: self.book,
            trades: self.trades,
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
        if self.current_position_idx > MOVE_SPEED {
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
            100,
            200,
            Point(0, 0),
            Point(2, 0),
            TradeBook::default(),
        )
        .unwrap();
        assert_eq!(caravan.sender_id, 100);
        assert_eq!(caravan.receiver_id, 200);
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
                    100,
                    200,
                    Point(0, 0),
                    end,
                    TradeBook::default(),
                )
                .is_none()
            );
        }
    }

    #[test]
    fn unreachable_cargo_is_handed_back() {
        let mut map = Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![0; 3]);
        map.terrain[1].elevation_val = 0.0;
        let mut finder = PathFinder::new(3, 1);
        let cargo = CaravanCargo {
            sender_id: 1,
            receiver_id: 2,
            book: TradeBook::new(7, Default::default()),
        };
        let cargo =
            Caravan::from_cargo(&map, &mut finder, cargo, Point(0, 0), Point(2, 0)).unwrap_err();
        assert_eq!(cargo.book.gold(), 7);
    }

    #[test]
    fn short_round_trip_returns_home_without_underflow() {
        let map = Map::new(5, 1, vec![Area::new(0.5, 0.5); 5], vec![0; 5]);
        let mut finder = PathFinder::new(5, 1);
        let mut caravan = Caravan::new(
            &map,
            &mut finder,
            1,
            2,
            Point(0, 0),
            Point(4, 0),
            TradeBook::default(),
        )
        .unwrap();
        caravan.travel();
        assert_eq!(caravan.state(), CaravanState::Trading);
        assert_eq!(caravan.position(), Point(4, 0));
        caravan.trade(&mut TradeBook::default());
        assert_eq!(caravan.state(), CaravanState::TravelBack);
        caravan.travel();
        assert_eq!(caravan.state(), CaravanState::Back);
        assert_eq!(caravan.position(), Point(0, 0));
        assert_eq!(caravan.into_return().outcome, CaravanOutcome::Traded);
    }
}
