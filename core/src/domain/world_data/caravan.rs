use crate::{domain::{trade::trade_log::TradeLog, world_data::{point::Point, world::World}}, services::algorithms::{a_star::AStar, path_finder::PathFinder}};
const MOVE_SPEED: usize = 10;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaravanState {
    TravelTo,
    Trading,
    TravelBack,
    Back,
}

pub struct Caravan {
    pub sender_id: usize,
    pub receiver_id: usize,
    pub start: Point,
    pub end: Point,
    pub current_position: Point,
    pub current_position_idx: usize,
    pub caravan_state: CaravanState,
    pub path: Vec<Point>,
    pub trade_log: TradeLog,
}

impl Caravan {
    pub fn new(world: &World, path_finder: &mut PathFinder, sender_id: usize, receiver_id: usize, trade_log: TradeLog) -> Caravan {
        let start: Point = world.centroid(sender_id);
        let end: Point = world.centroid(receiver_id);
        let current_position: Point = start.clone();
        let current_position_idx: usize = 0;
        let caravan_state: CaravanState = CaravanState::TravelTo;
        let path: Vec<Point> = AStar::a_star(&world.map, path_finder, (start.0, start.1), (end.0, end.1)).unwrap();
        Caravan { sender_id, receiver_id, start, end, current_position, current_position_idx, caravan_state, path, trade_log }
    }
    pub fn travel(&mut self) {
        match self.caravan_state {
            CaravanState::TravelTo => self.travel_to(),
            CaravanState::TravelBack =>self.travel_back(),
            _ => {}
        }
    }
    pub fn trade(&mut self, trade_log: &mut TradeLog) {
        if self.caravan_state == CaravanState::Trading {
            trade_log.trade(&mut self.trade_log);
            self.caravan_state = CaravanState::TravelBack;
        }
    }
    fn travel_to(&mut self) {
        if self.current_position_idx + MOVE_SPEED < self.path.len() - 1 {
            self.current_position_idx += MOVE_SPEED;
            self.current_position = self.path[self.current_position_idx];
        }
        else {
            self.current_position_idx = self.path.len() - 1;
            self.current_position = self.path[self.current_position_idx];
            self.caravan_state = CaravanState::Trading;
        }
    }
    fn travel_back(&mut self) {
        if 0 < self.current_position_idx - MOVE_SPEED {
            self.current_position_idx -= MOVE_SPEED;
            self.current_position = self.path[self.current_position_idx];
        }
        else {
            self.current_position_idx = 0;
            self.current_position = self.path[self.current_position_idx];
            self.caravan_state = CaravanState::Back;
        }
    }
}