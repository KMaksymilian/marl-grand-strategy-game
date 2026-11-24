//! Trade phases of the settlement turn: reservation, caravan departures,
//! market trading with visitors and collection of returns.
use crate::domain::economy::ResourceType;
use crate::domain::intelligence::{
    MarketObservation, MarketObservationSource, ObservationStrategy,
};
use crate::domain::settlement::Settlement;
use crate::domain::settlement::turn_report::{
    DispatchedCaravan, MarketTrade, MaterialTotals, ReturnedCaravan, TradeTurnReport,
};
use crate::domain::trade::allocation::TradeAllocationEngine;
use crate::domain::trade::book::TradeBook;
use crate::domain::trade::caravan::{Caravan, CaravanOutcome, CaravanReturn, CaravanState};
use crate::domain::trade::offer::TradeOffer;
use std::collections::HashMap;

fn totals<'a>(goods: impl IntoIterator<Item = (&'a ResourceType, &'a u32)>) -> MaterialTotals {
    goods
        .into_iter()
        .filter(|(resource, amount)| **resource != ResourceType::Labour && **amount > 0)
        .map(|(resource, &amount)| (resource.clone(), u64::from(amount)))
        .collect()
}

impl Settlement {
    /// Restore accepted escrow without discarding it if the warehouse or treasury filled up.
    pub(in crate::domain::settlement) fn refund_canceled_order(
        &mut self,
        mut caravan: CaravanReturn,
    ) -> ReturnedCaravan {
        let (goods, gold) = caravan.book.return_resources();
        let mut admitted = totals(&goods);
        let refused = self.inventory_manager.main.add_batch(
            goods
                .into_iter()
                .map(|(resource, amount)| (resource, u64::from(amount)))
                .collect(),
        );
        let remaining_offers: HashMap<_, _> = refused
            .into_iter()
            .map(|refusal| {
                *admitted
                    .get_mut(&refusal.resource)
                    .expect("refused return came from escrow") -= refusal.amount;
                (
                    refusal.resource,
                    TradeOffer::sell(
                        u32::try_from(refusal.amount).expect("refund came from stock"),
                        0,
                    ),
                )
            })
            .collect();
        admitted.retain(|_, amount| *amount > 0);
        let credited = gold.min(u32::MAX - self.gold());
        self.gold_inventory.add(credited);
        let receipt = ReturnedCaravan {
            receiver_id: caravan.receiver_id,
            outcome: caravan.outcome,
            trades: vec![],
            cargo: admitted,
            gold: u64::from(credited),
        };
        if !remaining_offers.is_empty() || gold > credited {
            caravan.book = TradeBook::new(gold - credited, remaining_offers);
            self.receive_caravan(caravan);
        }
        receipt
    }

    /// Moves stock and gold requested by the published offers into the market book.
    pub(in crate::domain::settlement) fn reserve_market(&mut self, report: &mut TradeTurnReport) {
        self.book.replace_offers(self.fundable_market_offers());
        let demands = self.book.calculate_demand();
        TradeAllocationEngine::execute(
            &mut self.book,
            &mut self.gold_inventory,
            &mut self.inventory_manager.main,
            demands,
        );
        report.market_reserved = totals(&self.book.goods_for_sale());
        report.market_gold_reserved = u64::from(self.book.gold());
    }

    /// Releases orders already funded at acceptance, in queue order.
    pub(in crate::domain::settlement) fn realize_caravan_orders(
        &mut self,
        report: &mut TradeTurnReport,
    ) {
        for order in std::mem::take(&mut self.trade.orders) {
            let cargo = order.into_cargo(self.id);
            report.caravans_dispatched.push(DispatchedCaravan {
                receiver_id: cargo.receiver_id,
                cargo: totals(&cargo.book.goods_for_sale()),
                gold: u64::from(cargo.book.gold()),
                route: None,
            });
            self.trade.dispatched.push(cargo);
        }
    }

    /// Trades the market book with caravans that arrived here; the book is shared equally.
    pub(in crate::domain::settlement) fn trade_with_visitors(
        &mut self,
        visitors: &mut [&mut Caravan],
        report: &mut TradeTurnReport,
        turn: u64,
        province_id: usize,
    ) {
        let mut guests: Vec<&mut Caravan> = visitors
            .iter_mut()
            .filter(|caravan| {
                caravan.state() == CaravanState::Trading && caravan.receiver_id == self.id
            })
            .map(|caravan| &mut **caravan)
            .collect();
        if guests.is_empty() {
            return;
        }
        let snapshot = MarketObservation.observe(
            turn,
            MarketObservationSource {
                market: self.id,
                province_id,
                book: &self.book,
            },
        );
        let log = {
            let mut books: Vec<&mut TradeBook> = guests
                .iter_mut()
                .map(|caravan| caravan.book_mut())
                .collect();
            self.book.trade_with_visitors(&mut books)
        };
        for (caravan, executions) in guests.into_iter().zip(log) {
            let caravan_sender_id = caravan.sender_id;
            report
                .market_trades
                .extend(executions.iter().cloned().map(|execution| MarketTrade {
                    caravan_sender_id,
                    execution,
                }));
            caravan.finish_trading(executions, snapshot.clone());
        }
    }

    /// Drains the market book and returned caravans. Gold goes straight to the treasury;
    /// goods are returned as one batch that the turn stores together with production.
    pub(in crate::domain::settlement) fn collect_trade_returns(
        &mut self,
        report: &mut TradeTurnReport,
    ) -> HashMap<ResourceType, u64> {
        let mut batch: HashMap<ResourceType, u64> = HashMap::new();
        let mut add = |goods: &HashMap<ResourceType, u32>| {
            for (resource, &amount) in goods {
                if *resource != ResourceType::Labour && amount > 0 {
                    *batch.entry(resource.clone()).or_default() += u64::from(amount);
                }
            }
        };

        report.market_bought = totals(&self.book.bought_goods());
        report.market_unsold = totals(&self.book.goods_for_sale());
        let (goods, gold) = self.book.return_resources();
        add(&goods);
        self.gold_inventory.add(gold);
        report.gold_received += u64::from(gold);

        let mut refunded = MaterialTotals::new();
        for mut caravan in std::mem::take(&mut self.trade.returning) {
            if caravan.outcome == CaravanOutcome::OrderCanceled {
                let receipt = self.refund_canceled_order(caravan);
                if receipt.cargo.is_empty() && receipt.gold == 0 {
                    continue;
                }
                for (resource, &amount) in &receipt.cargo {
                    *refunded.entry(resource.clone()).or_default() += amount;
                }
                report.gold_received += receipt.gold;
                report.caravans_returned.push(receipt);
                continue;
            }
            for snapshot in caravan.observations {
                self.remember_observation(snapshot);
            }
            let (goods, gold) = caravan.book.return_resources();
            add(&goods);
            self.gold_inventory.add(gold);
            report.gold_received += u64::from(gold);
            report.caravans_returned.push(ReturnedCaravan {
                receiver_id: caravan.receiver_id,
                outcome: caravan.outcome,
                trades: caravan.trades,
                cargo: totals(&goods),
                gold: u64::from(gold),
            });
        }

        report.received = refunded;
        for (resource, &amount) in &batch {
            *report.received.entry(resource.clone()).or_default() += amount;
        }
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::SettlementId;
    use crate::domain::intelligence::{Information, InformationSnapshot};
    use crate::domain::settlement::TradeCommandError;
    use crate::domain::trade::caravan::{CaravanOutcome, CaravanReturn};
    use crate::domain::trade::offer::TradeOffer;
    use crate::domain::world::map::Map;
    use crate::domain::world::pathfinding::path_finder::PathFinder;
    use crate::domain::world::terrain::area::Area;
    use crate::test_support::material;
    use std::sync::Arc;

    fn turn(settlement: &mut Settlement) -> crate::domain::settlement::turn_report::TurnReport {
        let map = Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![0; 4]);
        settlement.process_turn_for_test(
            &mut crate::domain::world::settlement_environment::MapSettlementEnvironment::new(
                &map,
                &mut PathFinder::new(4, 1),
                &mut crate::domain::world::territory::TerritoryManager::default(),
            ),
            &mut [],
        )
    }

    #[test]
    fn late_return_keeps_the_newer_market_observation() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        let market = SettlementId(2);
        let snapshot = |observed_turn, province_id| {
            InformationSnapshot::new(
                observed_turn,
                Information::ForeignMarket {
                    market,
                    province_id,
                    offers: Default::default(),
                },
            )
        };
        let latest = snapshot(5, 20);
        for observation in [snapshot(3, 10), latest.clone(), snapshot(4, 10)] {
            settlement.receive_caravan(CaravanReturn {
                receiver_id: market,
                outcome: CaravanOutcome::Traded,
                book: TradeBook::default(),
                trades: vec![],
                observations: vec![observation],
            });
            turn(&mut settlement);
        }
        assert_eq!(
            settlement.known_markets().collect::<Vec<_>>(),
            vec![&latest]
        );
    }

    #[test]
    fn unsold_market_stock_and_gold_return_each_turn_and_stay_published() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        settlement.deposit(material("wood"), 5);
        settlement.deposit_gold(20);
        settlement
            .publish_trade_offers(HashMap::from([
                (material("wood"), TradeOffer::sell(8, 2)),
                (material("stone"), TradeOffer::buy(4, 3)),
            ]))
            .unwrap();
        for _ in 0..2 {
            let report = turn(&mut settlement);
            assert_eq!(report.trade.market_reserved[&material("wood")], 5);
            assert_eq!(report.trade.market_gold_reserved, 12);
            assert_eq!(report.trade.market_unsold[&material("wood")], 5);
            assert_eq!(report.trade.gold_received, 12);
            assert!(report.inventory.losses.is_empty());
            assert!(report.inventory.is_balanced());
            assert_eq!(settlement.inventory().amount(material("wood")), 5);
            assert_eq!(settlement.gold(), 20);
        }
    }

    #[test]
    fn caravan_orders_are_funded_before_the_turn_and_depart_in_queue_order() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        settlement.deposit(material("grain"), 10);
        settlement
            .publish_trade_offers(HashMap::from([(material("grain"), TradeOffer::sell(6, 1))]))
            .unwrap();
        for (receiver, amount) in [(2, 3), (3, 1)] {
            settlement
                .order_caravan(
                    SettlementId(receiver),
                    HashMap::from([(material("grain"), TradeOffer::sell(amount, 1))]),
                )
                .unwrap();
        }
        assert_eq!(settlement.inventory().amount(material("grain")), 6);
        assert_eq!(
            settlement.order_caravan(
                SettlementId(4),
                HashMap::from([(material("grain"), TradeOffer::sell(1, 1))]),
            ),
            Err(TradeCommandError::InsufficientStock {
                resource: material("grain"),
                requested: 1,
                available: 0,
            }),
        );
        let report = turn(&mut settlement);
        let dispatched = &report.trade.caravans_dispatched;
        assert_eq!(dispatched.len(), 2);
        assert_eq!(dispatched[0].receiver_id, SettlementId(2));
        assert_eq!(dispatched[0].cargo[&material("grain")], 3);
        assert_eq!(dispatched[1].receiver_id, SettlementId(3));
        assert_eq!(dispatched[1].cargo[&material("grain")], 1);
        assert_eq!(report.inventory.opening_stock[&material("grain")], 10);
        assert_eq!(report.inventory.trade_reserved[&material("grain")], 10);
        assert!(report.inventory.is_balanced());
        assert_eq!(settlement.inventory().amount(material("grain")), 6);
        assert_eq!(settlement.take_dispatched_caravans().len(), 2);
        assert!(settlement.caravan_orders().is_empty());
    }

    #[test]
    fn funded_market_limits_set_priority_without_invalidating_accepted_caravans() {
        use crate::domain::buildings::passive::TradeLimitBonuses;

        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        let wood = material("wood");
        settlement.deposit(wood.clone(), 10);
        let mut limits = TradeLimitBonuses {
            caravan_slots: 1,
            caravan_cargo: 6,
            offer_slots: 1,
            open_offer_units: 10,
        };
        settlement.set_base_trade_limits(Some(limits));
        settlement
            .publish_trade_offers(HashMap::from([(wood.clone(), TradeOffer::sell(10, 1))]))
            .unwrap();
        limits.open_offer_units = 4;
        settlement.set_base_trade_limits(Some(limits));
        settlement
            .order_caravan(
                SettlementId(2),
                HashMap::from([(wood.clone(), TradeOffer::sell(6, 1))]),
            )
            .unwrap();

        limits.caravan_slots = 0;
        settlement.set_base_trade_limits(Some(limits));
        let report = turn(&mut settlement);
        assert_eq!(report.trade.market_reserved[&wood], 4);
        assert_eq!(report.trade.caravans_dispatched[0].cargo[&wood], 6);
        assert!(report.trade.unfunded_caravan_orders.is_empty());
        assert!(report.inventory.is_balanced());
    }

    #[test]
    fn canceled_escrow_survives_a_warehouse_and_treasury_filled_after_acceptance() {
        let mut settlement = Settlement::new(SettlementId(1), 5, 0, Arc::default());
        let wood = material("wood");
        let stone = material("stone");
        settlement.deposit(wood.clone(), 5);
        settlement.deposit_gold(1);
        settlement
            .order_caravan(
                SettlementId(2),
                HashMap::from([
                    (wood.clone(), TradeOffer::sell(5, 1)),
                    (stone.clone(), TradeOffer::buy(1, 1)),
                ]),
            )
            .unwrap();
        settlement.deposit(stone.clone(), 5);
        settlement.deposit_gold(u32::MAX);
        settlement.cancel_caravan_orders();
        assert!(settlement.caravan_orders().is_empty());
        assert_eq!(settlement.returning_caravans().len(), 1);
        assert_eq!(settlement.inventory().amount(wood.clone()), 0);
        assert!(turn(&mut settlement).trade.caravans_returned.is_empty());

        assert!(settlement.withdraw(stone.clone(), 3));
        assert!(settlement.gold_inventory.consume(1));
        let partial = turn(&mut settlement);
        assert_eq!(partial.inventory.trade_received[&wood], 3);
        assert_eq!(partial.trade.gold_received, 1);
        assert_eq!(
            partial.trade.caravans_returned[0].outcome,
            CaravanOutcome::OrderCanceled
        );
        assert!(partial.inventory.is_balanced());

        assert!(settlement.withdraw(stone, 2));
        let final_refund = turn(&mut settlement);
        assert_eq!(final_refund.inventory.trade_received[&wood], 2);
        assert_eq!(settlement.inventory().amount(wood), 5);
        assert_eq!(settlement.gold(), u32::MAX);
        assert!(final_refund.inventory.is_balanced());
        assert!(settlement.returning_caravans().is_empty());
    }

    fn partially_bought(units: u32, bought: u32, price: u32) -> TradeOffer {
        let mut offer = TradeOffer::buy(units, price);
        if let TradeOffer::Buy(buy) = &mut offer {
            buy.add_bought_amount(bought);
        }
        offer
    }

    #[test]
    fn published_offer_does_not_carry_earlier_purchases() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        settlement.deposit_gold(10);
        settlement
            .publish_trade_offers(HashMap::from([(
                material("wood"),
                partially_bought(3, 1, 2),
            )]))
            .unwrap();
        for _ in 0..3 {
            let report = turn(&mut settlement);
            assert!(report.trade.received.is_empty());
            assert!(report.inventory.is_balanced());
            assert_eq!(settlement.inventory().amount(material("wood")), 0);
            assert_eq!(settlement.gold(), 10);
        }
    }

    #[test]
    fn caravan_order_does_not_carry_earlier_purchases() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        settlement.deposit_gold(10);
        settlement
            .order_caravan(
                SettlementId(2),
                HashMap::from([(material("wood"), partially_bought(3, 1, 2))]),
            )
            .unwrap();
        turn(&mut settlement);
        let cargo = settlement.take_dispatched_caravans().pop().unwrap();
        assert!(cargo.book.bought_goods().is_empty());
    }

    #[test]
    fn invalid_trade_commands_are_rejected() {
        let mut settlement = Settlement::new(SettlementId(1), 50, 0, Arc::default());
        let labour = HashMap::from([(ResourceType::Labour, TradeOffer::sell(1, 1))]);
        assert_eq!(
            settlement.publish_trade_offers(labour.clone()),
            Err(TradeCommandError::LabourIsNotTradable)
        );
        assert_eq!(
            settlement.order_caravan(SettlementId(2), labour),
            Err(TradeCommandError::LabourIsNotTradable)
        );
        let wood = HashMap::from([(material("wood"), TradeOffer::buy(1, 1))]);
        assert_eq!(
            settlement.order_caravan(SettlementId(1), wood),
            Err(TradeCommandError::TradeWithSelf)
        );
        assert_eq!(
            settlement.order_caravan(
                SettlementId(2),
                HashMap::from([(material("wood"), TradeOffer::buy(0, 1))])
            ),
            Err(TradeCommandError::ZeroUnits(material("wood")))
        );
        assert_eq!(
            settlement.order_caravan(SettlementId(2), HashMap::new()),
            Err(TradeCommandError::EmptyOrder),
        );
        assert!(settlement.caravan_orders().is_empty());
    }
}
