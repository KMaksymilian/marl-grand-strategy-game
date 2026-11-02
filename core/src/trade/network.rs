//! World-owned settlements and caravans advance together in deterministic ID order.
use crate::settlement::turn_report::TurnReport;
use crate::world::pathfinding::path_finder::PathFinder;
use crate::world::{Point, World, WorldError};
use std::collections::HashMap;

pub struct TradeNetwork;
impl TradeNetwork {
    /// Preflight all registered settlements before movement, unloading or spending.
    /// Reports follow ascending settlement ID; locations provide explicit route endpoints.
    pub fn process_turn(
        world: &mut World,
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) -> Result<Vec<TurnReport>, WorldError> {
        let ids = world.settlement_ids();
        for &id in &ids {
            world.validate_settlement_turn(id)?;
        }
        world.begin_trade_turn();
        let mut reports = Vec::with_capacity(ids.len());
        for id in ids {
            reports.push(world.process_settlement_turn(id, finder)?);
        }
        world.finish_trade_turn(locations, finder);
        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::ResourceType;
    use crate::settlement::Settlement;
    use crate::settlement::turn_report::{InventoryLoss, InventoryLossStage};
    use crate::trade::caravan::{CaravanOutcome, CaravanState};
    use crate::trade::execution::{TradeExecution, TradeSide};
    use crate::trade::offer::TradeOffer;
    use crate::world::Province;
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
            (0..WIDTH * HEIGHT).map(|index| index / WIDTH + 1).collect(),
        );
        World::new(map, (1..=HEIGHT).map(Province::new).collect())
    }

    fn settlement(id: u32, capacity: u32) -> Settlement {
        Settlement::new(id, capacity, 0, Arc::default())
    }

    fn run_until_idle(
        world: &mut World,
        locations: &HashMap<u32, Point>,
        finder: &mut PathFinder,
    ) -> Vec<Vec<TurnReport>> {
        let mut history = Vec::new();
        for _ in 0..20 {
            history.push(TradeNetwork::process_turn(world, locations, finder).unwrap());
            if world.caravans.is_empty()
                && world
                    .settlements()
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
        for settlement in [seller, buyer] {
            world
                .add_settlement(settlement.id() as usize, settlement)
                .unwrap();
        }

        let history = run_until_idle(&mut world, &locations, &mut finder);
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

        let seller = world.settlement(1).unwrap();
        let buyer = world.settlement(2).unwrap();
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
        for settlement in [first, second, market] {
            world
                .add_settlement(settlement.id() as usize, settlement)
                .unwrap();
        }

        let history = run_until_idle(&mut world, &locations, &mut finder);
        let market_trades: Vec<_> = history
            .iter()
            .flat_map(|r| r[2].trade.market_trades.clone())
            .collect();
        assert_eq!(market_trades.len(), 2);
        assert!(market_trades.iter().all(|t| t.execution.units == 4));
        assert_eq!(
            world
                .settlement(1)
                .unwrap()
                .inventory()
                .amount(ResourceType::Grain),
            4
        );
        assert_eq!(
            world
                .settlement(2)
                .unwrap()
                .inventory()
                .amount(ResourceType::Grain),
            4
        );
        assert_eq!(world.settlement(1).unwrap().gold(), 88);
        assert_eq!(world.settlement(3).unwrap().gold(), 24);
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
        for settlement in [sender, buyer] {
            world
                .add_settlement(settlement.id() as usize, settlement)
                .unwrap();
        }

        // Turn 1 funds the caravan, emptying the warehouse; refill it while it travels.
        let first = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert_eq!(first[0].inventory.trade_reserved[&ResourceType::Stone], 10);
        world
            .settlement_mut(1)
            .unwrap()
            .deposit(ResourceType::Wood, 7);

        let history = run_until_idle(&mut world, &locations, &mut finder);
        let report = history
            .iter()
            .map(|r| &r[0])
            .find(|r| !r.trade.caravans_returned.is_empty())
            .unwrap();
        assert_eq!(
            report.trade.caravans_returned[0].cargo[&ResourceType::Stone],
            10
        );
        assert!(report.inventory.losses.contains(&InventoryLoss {
            resource: ResourceType::Stone,
            amount: 7,
            stage: InventoryLossStage::Warehouse,
        }));
        assert!(report.inventory.is_balanced());
        assert_eq!(
            world
                .settlement(1)
                .unwrap()
                .inventory()
                .amount(ResourceType::Stone),
            3
        );
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
        for settlement in [sender, settlement(2, 100)] {
            world
                .add_settlement(settlement.id() as usize, settlement)
                .unwrap();
        }

        let first = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert_eq!(first[0].trade.caravans_dispatched.len(), 1);
        assert_eq!(first[0].trade.unfunded_caravan_orders, vec![9]);
        assert!(world.caravans.is_empty());

        let second = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        let returned = &second[0].trade.caravans_returned;
        assert_eq!(returned.len(), 1);
        assert_eq!(returned[0].outcome, CaravanOutcome::NoRoute);
        assert_eq!(world.settlement(1).unwrap().gold(), 30);
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
        for settlement in [seller, buyer] {
            world
                .add_settlement(settlement.id() as usize, settlement)
                .unwrap();
        }

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
            let reports = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
            assert_eq!(world.caravans.len(), 1, "turn {turn}");
            assert_eq!(world.caravans[0].state(), state, "turn {turn}");
            assert_eq!(world.caravans[0].position(), position, "turn {turn}");
            assert_eq!(reports[1].trade.market_trades.len(), trades, "turn {turn}");
            assert!(reports.iter().all(|r| r.inventory.is_balanced()));
        }
        // In transit the wood belongs to nobody's warehouse; the gold is in the caravan.
        assert_eq!(
            world
                .settlement(1)
                .unwrap()
                .inventory()
                .amount(ResourceType::Wood),
            0
        );
        assert_eq!(
            world
                .settlement(2)
                .unwrap()
                .inventory()
                .amount(ResourceType::Wood),
            10
        );
        assert_eq!(world.settlement(1).unwrap().gold(), 0);
        assert_eq!(world.caravans[0].book().gold(), 50);

        // Back at the origin: removed from the world and unloaded in the same turn.
        let reports = TradeNetwork::process_turn(&mut world, &locations, &mut finder).unwrap();
        assert!(world.caravans.is_empty());
        assert_eq!(reports[0].trade.caravans_returned.len(), 1);
        assert_eq!(reports[0].trade.trade_count(), 1);
        assert_eq!(world.settlement(1).unwrap().gold(), 50);
        assert_eq!(world.settlement(2).unwrap().gold(), 50);
    }
}
