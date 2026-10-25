//! One world turn of settlements connected by caravans.
//!
//! Order of a turn:
//! 1. Caravans move (`World::caravan_travel`).
//! 2. Caravans that reached home leave the world and are handed to their sender.
//! 3. Caravans waiting at a missing receiver turn back without trading.
//! 4. Every settlement runs its turn. Its market book is shared equally between the
//!    caravans waiting there; the turn also unloads the caravans from step 2.
//! 5. Caravans funded during the turn are routed and added to the world. Cargo
//!    without a route or receiver goes back to the sender and is unloaded next turn.
use crate::settlement::Settlement;
use crate::settlement::turn_report::TurnReport;
use crate::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::world::pathfinding::path_finder::PathFinder;
use crate::world::{Point, World};
use std::collections::{HashMap, HashSet};

pub struct TradeNetwork;

impl TradeNetwork {
    /// Runs one turn for all settlements; reports are in `settlements` order.
    /// `locations` gives each settlement's map point used as caravan start/end.
    pub fn process_turn(
        world: &mut World,
        settlements: &mut [Settlement],
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) -> Vec<TurnReport> {
        world.caravan_travel();
        Self::unload_returned(world, settlements);
        Self::turn_back_unserved(world, settlements);
        let reports = Self::run_settlement_turns(world, settlements, finder);
        Self::dispatch(world, settlements, locations, finder);
        reports
    }

    fn unload_returned(world: &mut World, settlements: &mut [Settlement]) {
        let (back, travelling): (Vec<_>, Vec<_>) = std::mem::take(&mut world.caravans)
            .into_iter()
            .partition(|caravan| caravan.state() == CaravanState::Back);
        world.caravans = travelling;
        for caravan in back {
            let sender = caravan.sender_id as u32;
            // A caravan whose sender no longer exists is dropped with its cargo.
            if let Some(settlement) = settlements.iter_mut().find(|s| s.id() == sender) {
                settlement.receive_caravan(caravan.into_return());
            }
        }
    }

    fn turn_back_unserved(world: &mut World, settlements: &[Settlement]) {
        let ids: HashSet<usize> = settlements.iter().map(|s| s.id() as usize).collect();
        for caravan in &mut world.caravans {
            if caravan.state() == CaravanState::Trading && !ids.contains(&caravan.receiver_id) {
                caravan.abort_trading();
            }
        }
    }

    fn run_settlement_turns(
        world: &mut World,
        settlements: &mut [Settlement],
        finder: &mut PathFinder,
    ) -> Vec<TurnReport> {
        let World { map, caravans, .. } = world;
        settlements
            .iter_mut()
            .map(|settlement| {
                let id = settlement.id() as usize;
                let mut visitors: Vec<&mut Caravan> = caravans
                    .iter_mut()
                    .filter(|c| c.state() == CaravanState::Trading && c.receiver_id == id)
                    .collect();
                // Stable order makes leftover distribution deterministic.
                visitors.sort_by_key(|caravan| caravan.sender_id);
                settlement.process_turn_with_visitors(map, finder, &mut visitors)
            })
            .collect()
    }

    fn dispatch(
        world: &mut World,
        settlements: &mut [Settlement],
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) {
        let ids: HashSet<u32> = settlements.iter().map(Settlement::id).collect();
        for settlement in settlements.iter_mut() {
            for cargo in settlement.take_dispatched_caravans() {
                if !ids.contains(&cargo.receiver_id) {
                    settlement.receive_caravan(CaravanReturn::undelivered(
                        cargo,
                        CaravanOutcome::ReceiverMissing,
                    ));
                    continue;
                }
                let route = locations
                    .get(&cargo.sender_id)
                    .zip(locations.get(&cargo.receiver_id));
                let Some((&start, &end)) = route else {
                    settlement.receive_caravan(CaravanReturn::undelivered(
                        cargo,
                        CaravanOutcome::NoRoute,
                    ));
                    continue;
                };
                match Caravan::from_cargo(&world.map, finder, cargo, start, end) {
                    Ok(caravan) => world.caravans.push(caravan),
                    Err(cargo) => settlement.receive_caravan(CaravanReturn::undelivered(
                        cargo,
                        CaravanOutcome::NoRoute,
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::ResourceType;
    use crate::settlement::turn_report::{InventoryLoss, InventoryLossStage};
    use crate::trade::execution::{TradeExecution, TradeSide};
    use crate::trade::offer::TradeOffer;
    use crate::world::map::Map;
    use crate::world::terrain::area::Area;
    use std::sync::Arc;

    const WIDTH: usize = 64;
    const HEIGHT: usize = 4;

    fn world() -> World {
        let map = Map::new(
            WIDTH,
            HEIGHT,
            vec![Area::new(0.5, 0.5); WIDTH * HEIGHT],
            vec![0; WIDTH * HEIGHT],
        );
        World::new(map, Vec::new())
    }

    fn settlement(id: u32, capacity: u32) -> Settlement {
        Settlement::new(id, capacity, 0, Arc::default())
    }

    fn run_until_idle(
        world: &mut World,
        settlements: &mut [Settlement],
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) -> Vec<Vec<TurnReport>> {
        let mut history = Vec::new();
        for _ in 0..20 {
            history.push(TradeNetwork::process_turn(
                world,
                settlements,
                locations,
                finder,
            ));
            if world.caravans.is_empty()
                && settlements
                    .iter()
                    .all(|s| s.returning_caravans().is_empty())
            {
                return history;
            }
        }
        panic!("caravans did not finish");
    }

    #[test]
    fn caravan_sells_abroad_and_brings_gold_home() {
        let mut world = world();
        let mut finder = PathFinder::new(WIDTH, HEIGHT);
        let locations = HashMap::from([(1, Point(0, 0)), (2, Point(25, 0))]);
        let mut seller = settlement(1, 100);
        let mut buyer = settlement(2, 100);
        seller.deposit(ResourceType::Wood, 10);
        buyer.deposit_gold(100);
        buyer
            .publish_trade_offers(HashMap::from([(
                ResourceType::Wood,
                TradeOffer::buy(6, 10),
            )]))
            .unwrap();
        seller
            .order_caravan(
                2,
                HashMap::from([(ResourceType::Wood, TradeOffer::sell(8, 4))]),
            )
            .unwrap();
        let mut settlements = [seller, buyer];

        let history = run_until_idle(&mut world, &mut settlements, &locations, &mut finder);
        for reports in &history {
            assert!(reports.iter().all(|r| r.inventory.is_balanced()));
        }

        let dispatched = &history[0][0].trade.caravans_dispatched;
        assert_eq!(dispatched.len(), 1);
        assert_eq!(dispatched[0].cargo[&ResourceType::Wood], 8);

        // Buyer: 6 wood at midpoint price 7 at its own market.
        let market: Vec<_> = history
            .iter()
            .flat_map(|r| r[1].trade.market_trades.clone())
            .collect();
        assert_eq!(market.len(), 1);
        assert_eq!(market[0].caravan_sender_id, 1);
        assert_eq!(
            market[0].execution,
            TradeExecution {
                resource: ResourceType::Wood,
                units: 6,
                unit_price: 7,
                side: TradeSide::Bought,
            }
        );

        let returned: Vec<_> = history
            .iter()
            .flat_map(|r| r[0].trade.caravans_returned.clone())
            .collect();
        assert_eq!(returned.len(), 1);
        assert_eq!(returned[0].outcome, CaravanOutcome::Traded);
        assert_eq!(returned[0].gold, 42);
        assert_eq!(returned[0].cargo[&ResourceType::Wood], 2);
        assert_eq!(returned[0].trades[0].side, TradeSide::Sold);

        let [seller, buyer] = &settlements;
        assert_eq!(seller.gold(), 42);
        assert_eq!(seller.inventory().amount(ResourceType::Wood), 4);
        assert_eq!(buyer.gold(), 58);
        assert_eq!(buyer.inventory().amount(ResourceType::Wood), 6);
    }

    #[test]
    fn two_caravans_at_one_market_share_it_equally() {
        let mut world = world();
        let mut finder = PathFinder::new(WIDTH, HEIGHT);
        // Both senders are equally far from the market.
        let locations = HashMap::from([(1, Point(0, 0)), (2, Point(40, 0)), (3, Point(20, 0))]);
        let mut first = settlement(1, 100);
        let mut second = settlement(2, 100);
        let mut market = settlement(3, 100);
        for buyer in [&mut first, &mut second] {
            buyer.deposit_gold(100);
            buyer
                .order_caravan(
                    3,
                    HashMap::from([(ResourceType::Grain, TradeOffer::buy(10, 3))]),
                )
                .unwrap();
        }
        market.deposit(ResourceType::Grain, 8);
        market
            .publish_trade_offers(HashMap::from([(
                ResourceType::Grain,
                TradeOffer::sell(8, 3),
            )]))
            .unwrap();
        let mut settlements = [first, second, market];

        let history = run_until_idle(&mut world, &mut settlements, &locations, &mut finder);
        let market_trades: Vec<_> = history
            .iter()
            .flat_map(|r| r[2].trade.market_trades.clone())
            .collect();
        assert_eq!(market_trades.len(), 2);
        assert!(market_trades.iter().all(|t| t.execution.units == 4));
        assert_eq!(settlements[0].inventory().amount(ResourceType::Grain), 4);
        assert_eq!(settlements[1].inventory().amount(ResourceType::Grain), 4);
        assert_eq!(settlements[0].gold(), 88);
        assert_eq!(settlements[2].gold(), 24);
        for reports in &history {
            assert!(reports.iter().all(|r| r.inventory.is_balanced()));
        }
    }

    #[test]
    fn goods_that_do_not_fit_on_return_are_reported_as_lost() {
        let mut world = world();
        let mut finder = PathFinder::new(WIDTH, HEIGHT);
        let locations = HashMap::from([(1, Point(0, 0)), (2, Point(5, 0))]);
        let mut sender = settlement(1, 10);
        let buyer = settlement(2, 100);
        sender.deposit(ResourceType::Stone, 10);
        sender
            .order_caravan(
                2,
                HashMap::from([(ResourceType::Stone, TradeOffer::sell(10, 1))]),
            )
            .unwrap();
        let mut settlements = [sender, buyer];

        // Turn 1 funds the caravan, emptying the warehouse; refill it while it travels.
        let first =
            TradeNetwork::process_turn(&mut world, &mut settlements, &locations, &mut finder);
        assert_eq!(first[0].inventory.trade_reserved[&ResourceType::Stone], 10);
        settlements[0].deposit(ResourceType::Wood, 7);

        let history = run_until_idle(&mut world, &mut settlements, &locations, &mut finder);
        let report = history
            .iter()
            .map(|r| &r[0])
            .find(|r| !r.trade.caravans_returned.is_empty())
            .unwrap();
        assert_eq!(
            report.trade.caravans_returned[0].cargo[&ResourceType::Stone],
            10
        );
        assert_eq!(report.trade.lost[&ResourceType::Stone], 7);
        assert!(report.inventory.losses.contains(&InventoryLoss {
            resource: ResourceType::Stone,
            amount: 7,
            stage: InventoryLossStage::TradeReturns,
        }));
        assert!(report.inventory.is_balanced());
        assert_eq!(settlements[0].inventory().amount(ResourceType::Stone), 3);
    }

    #[test]
    fn cargo_without_route_or_receiver_comes_back_next_turn() {
        let mut world = world();
        let mut finder = PathFinder::new(WIDTH, HEIGHT);
        let locations = HashMap::from([(1, Point(0, 0))]);
        let mut sender = settlement(1, 100);
        sender.deposit_gold(30);
        sender
            .order_caravan(
                2,
                HashMap::from([(ResourceType::Wood, TradeOffer::buy(10, 3))]),
            )
            .unwrap();
        sender
            .order_caravan(
                9,
                HashMap::from([(ResourceType::Wood, TradeOffer::buy(10, 3))]),
            )
            .unwrap();
        let mut settlements = [sender, settlement(2, 100)];

        let first =
            TradeNetwork::process_turn(&mut world, &mut settlements, &locations, &mut finder);
        assert_eq!(first[0].trade.caravans_dispatched.len(), 1);
        assert_eq!(first[0].trade.unfunded_caravan_orders, vec![9]);
        assert!(world.caravans.is_empty());

        let second =
            TradeNetwork::process_turn(&mut world, &mut settlements, &locations, &mut finder);
        let returned = &second[0].trade.caravans_returned;
        assert_eq!(returned.len(), 1);
        assert_eq!(returned[0].outcome, CaravanOutcome::NoRoute);
        assert_eq!(settlements[0].gold(), 30);
    }

    #[test]
    fn caravan_moves_on_the_world_map_and_trades_only_on_arrival() {
        let mut world = world();
        let mut finder = PathFinder::new(WIDTH, HEIGHT);
        let locations = HashMap::from([(1, Point(0, 0)), (2, Point(25, 0))]);
        let mut seller = settlement(1, 100);
        let mut buyer = settlement(2, 100);
        seller.deposit(ResourceType::Wood, 10);
        buyer.deposit_gold(100);
        buyer
            .publish_trade_offers(HashMap::from([(
                ResourceType::Wood,
                TradeOffer::buy(10, 5),
            )]))
            .unwrap();
        seller
            .order_caravan(
                2,
                HashMap::from([(ResourceType::Wood, TradeOffer::sell(10, 5))]),
            )
            .unwrap();
        let mut settlements = [seller, buyer];

        // (state after the turn, caravan position, trades at the buyer's market)
        let expected = [
            (CaravanState::TravelTo, Point(0, 0), 0),
            (CaravanState::TravelTo, Point(10, 0), 0),
            (CaravanState::TravelTo, Point(20, 0), 0),
            (CaravanState::TravelBack, Point(25, 0), 1),
            (CaravanState::TravelBack, Point(15, 0), 0),
            (CaravanState::TravelBack, Point(5, 0), 0),
        ];
        for (turn, (state, position, trades)) in expected.into_iter().enumerate() {
            let reports =
                TradeNetwork::process_turn(&mut world, &mut settlements, &locations, &mut finder);
            assert_eq!(world.caravans.len(), 1, "turn {turn}");
            assert_eq!(world.caravans[0].state(), state, "turn {turn}");
            assert_eq!(world.caravans[0].position(), position, "turn {turn}");
            assert_eq!(reports[1].trade.market_trades.len(), trades, "turn {turn}");
            assert!(reports.iter().all(|r| r.inventory.is_balanced()));
        }
        // In transit the wood belongs to nobody's warehouse; the gold is in the caravan.
        assert_eq!(settlements[0].inventory().amount(ResourceType::Wood), 0);
        assert_eq!(settlements[1].inventory().amount(ResourceType::Wood), 10);
        assert_eq!(settlements[0].gold(), 0);
        assert_eq!(world.caravans[0].book().gold(), 50);

        // Back at the origin: removed from the world and unloaded in the same turn.
        let reports =
            TradeNetwork::process_turn(&mut world, &mut settlements, &locations, &mut finder);
        assert!(world.caravans.is_empty());
        assert_eq!(reports[0].trade.caravans_returned.len(), 1);
        assert_eq!(reports[0].trade.trade_count(), 1);
        assert_eq!(settlements[0].gold(), 50);
        assert_eq!(settlements[1].gold(), 50);
    }
}
