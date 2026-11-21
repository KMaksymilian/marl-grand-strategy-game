//! Caravan transport coordinates the same settlement instances as construction.
use super::World;
use crate::domain::ids::SettlementId;
use crate::domain::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::domain::world::{Point, pathfinding::path_finder::PathFinder};
use std::collections::HashMap;

impl World {
    pub(crate) fn prepare_settlement_turn(
        &mut self,
        id: SettlementId,
        finder: &mut PathFinder,
        turn: u64,
    ) -> crate::domain::settlement::PreparedTurn {
        let province_id = self
            .settlement_province(id)
            .expect("registered settlement province");
        let mut visitors: Vec<_> = self
            .caravans
            .iter_mut()
            .filter(|caravan| caravan.state() == CaravanState::Trading && caravan.receiver_id == id)
            .collect();
        visitors.sort_by_key(|caravan| caravan.sender_id);
        self.settlements
            .get_mut(id)
            .expect("registered settlement")
            .prepare_turn(
                &mut crate::domain::world::settlement_environment::MapSettlementEnvironment::new(
                    &self.map,
                    finder,
                    &mut self.territory,
                )
                .with_province(province_id),
                &mut visitors,
                turn,
                province_id,
            )
    }

    pub(crate) fn capital_trade_locations(&self) -> HashMap<SettlementId, Point> {
        self.settlements()
            .map(|settlement| {
                let capital = settlement
                    .capital()
                    .expect("registered settlement has a capital");
                (
                    settlement.id(),
                    Point(capital.x as usize, capital.y as usize),
                )
            })
            .collect()
    }
    pub(crate) fn begin_trade_turn(&mut self) {
        self.caravan_travel();
        let (returned, travelling): (Vec<_>, Vec<_>) = std::mem::take(&mut self.caravans)
            .into_iter()
            .partition(|caravan| caravan.state() == CaravanState::Back);
        self.caravans = travelling;
        for caravan in returned {
            if let Ok(sender) = self.settlement_mut(caravan.sender_id) {
                sender.receive_caravan(caravan.try_into_return().expect("caravan back home"));
            }
        }
        let existing: std::collections::HashSet<_> = self.settlement_ids().into_iter().collect();
        // Decided before borrowing the caravans mutably: a war or embargo
        // declared since departure stops the trade at the receiver (§8.1).
        let blocked: Vec<bool> = self
            .caravans
            .iter()
            .map(|caravan| !self.trade_allowed(caravan.sender_id, caravan.receiver_id))
            .collect();
        for (caravan, blocked) in self.caravans.iter_mut().zip(blocked) {
            if caravan.state() != CaravanState::Trading {
                continue;
            }
            if !existing.contains(&caravan.receiver_id) {
                caravan.abort_trading(CaravanOutcome::ReceiverMissing);
            } else if blocked {
                caravan.abort_trading(CaravanOutcome::TradeBlocked);
            }
        }
    }

    pub(crate) fn finish_trade_turn(
        &mut self,
        locations: &HashMap<SettlementId, Point>,
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
                } else if !self.trade_allowed(cargo.sender_id, cargo.receiver_id) {
                    // Ordered before a war or embargo of this turn: it never leaves.
                    Some(CaravanOutcome::TradeBlocked)
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
