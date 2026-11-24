//! Caravan transport coordinates the same settlement instances as construction.
use super::World;
use crate::domain::buildings::passive::TradeLimitBonuses;
use crate::domain::economy::ResourceType;
use crate::domain::ids::SettlementId;
use crate::domain::settlement::turn_report::CaravanDepartureRoute;
use crate::domain::settlement::{CaravanUsage, TradeCommandError, offer_units};
use crate::domain::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::domain::trade::offer::TradeOffer;
use crate::domain::world::{Point, pathfinding::path_finder::PathFinder};
use std::collections::HashMap;

impl World {
    pub fn set_base_trade_limits(&mut self, limits: Option<TradeLimitBonuses>) {
        self.base_trade_limits = limits;
        for id in self.settlement_ids() {
            self.settlement_mut(id)
                .expect("registered settlement")
                .set_base_trade_limits(limits);
        }
    }

    pub(crate) fn active_caravan_usage(&self, id: SettlementId) -> CaravanUsage {
        let mut usage = CaravanUsage::default();
        for caravan in self
            .caravans
            .iter()
            .filter(|caravan| caravan.sender_id == id)
        {
            let book = caravan.book();
            let purchased: u64 = book
                .bought_goods()
                .values()
                .map(|&amount| u64::from(amount))
                .sum();
            usage.add(offer_units(book.offers()).saturating_add(purchased));
        }
        usage
    }

    /// Funds a caravan to another registered settlement. Routes are assigned at turn end.
    pub fn order_caravan(
        &mut self,
        sender_id: SettlementId,
        receiver_id: SettlementId,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> Result<(), TradeCommandError> {
        if self.settlement(sender_id).is_none() {
            return Err(TradeCommandError::UnknownSender(sender_id));
        }
        if self.settlement(receiver_id).is_none() {
            return Err(TradeCommandError::UnknownReceiver(receiver_id));
        }
        let usage = self.active_caravan_usage(sender_id);
        self.settlement_mut(sender_id)
            .expect("validated caravan sender")
            .order_caravan_with_usage(receiver_id, offers, usage)
    }

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
    ) -> HashMap<SettlementId, Vec<CaravanDepartureRoute>> {
        let mut routes = HashMap::new();
        for id in self.settlement_ids() {
            let departures = self
                .settlement_mut(id)
                .expect("registered settlement")
                .take_dispatched_caravans();
            for cargo in departures {
                let route = locations
                    .get(&cargo.sender_id)
                    .zip(locations.get(&cargo.receiver_id));
                let (status, returned) = if self.settlement(cargo.receiver_id).is_none() {
                    (
                        CaravanDepartureRoute::ReceiverMissing,
                        Some(CaravanReturn::undelivered(
                            cargo,
                            CaravanOutcome::ReceiverMissing,
                        )),
                    )
                } else if let Some((&start, &end)) = route {
                    match Caravan::from_cargo(&self.map, finder, cargo, start, end) {
                        Ok(caravan) => {
                            self.caravans.push(caravan);
                            (CaravanDepartureRoute::Routed, None)
                        }
                        Err(cargo) => (
                            CaravanDepartureRoute::NoRoute,
                            Some(CaravanReturn::undelivered(cargo, CaravanOutcome::NoRoute)),
                        ),
                    }
                } else {
                    (
                        CaravanDepartureRoute::NoRoute,
                        Some(CaravanReturn::undelivered(cargo, CaravanOutcome::NoRoute)),
                    )
                };
                routes.entry(id).or_insert_with(Vec::new).push(status);
                if let Some(returned) = returned {
                    self.settlement_mut(id)
                        .expect("registered sender")
                        .receive_caravan(returned);
                }
            }
        }
        routes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settlement::Settlement;
    use crate::domain::world::{Map, Position, Province, terrain::area::Area};
    use crate::test_support::material;
    use std::sync::Arc;

    #[test]
    fn world_order_checks_pending_and_live_caravans_against_capacity() {
        let mut world = World::new(
            Map::new(2, 1, vec![Area::new(0.5, 0.5); 2], vec![1, 2]),
            vec![Province::new(1), Province::new(2)],
        );
        world.set_base_trade_limits(Some(TradeLimitBonuses {
            caravan_slots: 1,
            caravan_cargo: 10,
            ..TradeLimitBonuses::default()
        }));
        let sender = SettlementId(1);
        let receiver = SettlementId(2);
        let mut settlement = Settlement::new(sender, 10, 0, Arc::default());
        let wood = material("wood");
        settlement.deposit(wood.clone(), 2);
        world
            .add_settlement_at(1, Position { x: 0, y: 0 }, settlement)
            .unwrap();
        world
            .add_settlement_at(
                2,
                Position { x: 1, y: 0 },
                Settlement::new(receiver, 10, 0, Arc::default()),
            )
            .unwrap();
        let offers = || HashMap::from([(wood.clone(), TradeOffer::sell(1, 1))]);
        world.order_caravan(sender, receiver, offers()).unwrap();
        assert_eq!(
            world.order_caravan(sender, receiver, offers()),
            Err(TradeCommandError::CaravanCapacityExceeded)
        );

        let mut finder = PathFinder::new(2, 1);
        world
            .process_settlement_turn_for_test(sender, &mut finder)
            .unwrap();
        let locations = world.capital_trade_locations();
        let departures = world.finish_trade_turn(&locations, &mut finder);
        assert_eq!(departures[&sender], [CaravanDepartureRoute::Routed]);
        assert_eq!(
            world.order_caravan(sender, receiver, offers()),
            Err(TradeCommandError::CaravanCapacityExceeded)
        );
        assert_eq!(
            world.settlement(sender).unwrap().inventory().amount(wood),
            1
        );
    }
}
