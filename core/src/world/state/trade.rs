//! Caravan transport coordinates the same settlement instances as construction.
use super::World;
use crate::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::world::{Point, pathfinding::path_finder::PathFinder};
use std::collections::HashMap;

impl World {
    pub(crate) fn begin_trade_turn(&mut self) {
        self.caravan_travel();
        let (returned, travelling): (Vec<_>, Vec<_>) = std::mem::take(&mut self.caravans)
            .into_iter()
            .partition(|caravan| caravan.state() == CaravanState::Back);
        self.caravans = travelling;
        for caravan in returned {
            if let Ok(sender) = self.settlement_mut(caravan.sender_id as u32) {
                sender.receive_caravan(caravan.into_return());
            }
        }
        for caravan in &mut self.caravans {
            if caravan.state() == CaravanState::Trading
                && !self.settlements.contains_key(&(caravan.receiver_id as u32))
            {
                caravan.abort_trading();
            }
        }
    }

    pub(crate) fn finish_trade_turn(
        &mut self,
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) {
        for id in self.settlement_ids() {
            let departures = self
                .settlement_mut(id)
                .expect("registered settlement")
                .take_dispatched_caravans();
            for cargo in departures {
                let outcome = if self.settlement(cargo.receiver_id).is_none() {
                    Some(CaravanOutcome::ReceiverMissing)
                } else {
                    None
                };
                let route = locations
                    .get(&cargo.sender_id)
                    .zip(locations.get(&cargo.receiver_id));
                let returned = if let Some(outcome) = outcome {
                    Some(CaravanReturn::undelivered(cargo, outcome))
                } else if let Some((&start, &end)) = route {
                    match Caravan::from_cargo(&self.map, finder, cargo, start, end) {
                        Ok(caravan) => {
                            self.caravans.push(caravan);
                            None
                        }
                        Err(cargo) => {
                            Some(CaravanReturn::undelivered(cargo, CaravanOutcome::NoRoute))
                        }
                    }
                } else {
                    Some(CaravanReturn::undelivered(cargo, CaravanOutcome::NoRoute))
                };
                if let Some(returned) = returned {
                    self.settlement_mut(id)
                        .expect("registered sender")
                        .receive_caravan(returned);
                }
            }
        }
    }
}
