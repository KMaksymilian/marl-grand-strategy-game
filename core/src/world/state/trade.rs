//! Caravan transport coordinates the same settlement instances as construction.
use super::World;
use crate::ids::SettlementId;
use crate::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::world::{Point, pathfinding::path_finder::PathFinder};
use std::collections::HashMap;

impl World {
    pub(crate) fn prepare_settlement_turn(
        &mut self,
        id: SettlementId,
        finder: &mut PathFinder,
    ) -> crate::settlement::PreparedTurn {
        let mut visitors: Vec<_> = self
            .caravans
            .iter_mut()
            .filter(|caravan| caravan.state() == CaravanState::Trading && caravan.receiver_id == id)
            .collect();
        visitors.sort_by_key(|caravan| caravan.sender_id);
        self.settlements
            .get_mut(id)
            .expect("registered settlement")
            .prepare_turn(&self.map, finder, &mut self.territory, &mut visitors)
    }

    /// First land tile of each registered province, in row-major order.
    /// No implicit province-ID-to-coordinate conversion; no endpoint means NoRoute.
    pub(crate) fn default_trade_locations(&self) -> HashMap<SettlementId, Point> {
        let mut locations = HashMap::new();
        for (index, &province) in self.map.territory.iter().enumerate() {
            let Some(&id) = self.province_settlements.get(&province) else {
                continue;
            };
            if self.map.terrain.get(index).is_some_and(|tile| {
                tile.determine_elevation() != crate::world::terrain::elevation::Elevation::Ocean
            }) {
                locations
                    .entry(id)
                    .or_insert(Point(index % self.map.width, index / self.map.width));
            }
        }
        locations
    }
    pub(crate) fn begin_trade_turn(&mut self) {
        self.caravan_travel();
        let (returned, travelling): (Vec<_>, Vec<_>) = std::mem::take(&mut self.caravans)
            .into_iter()
            .partition(|caravan| caravan.state() == CaravanState::Back);
        self.caravans = travelling;
        for caravan in returned {
            if let Ok(sender) = self.settlement_mut(caravan.sender_id) {
                sender.receive_caravan(caravan.into_return());
            }
        }
        let existing: std::collections::HashSet<_> = self.settlement_ids().into_iter().collect();
        for caravan in &mut self.caravans {
            if caravan.state() == CaravanState::Trading && !existing.contains(&caravan.receiver_id)
            {
                caravan.abort_trading();
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
