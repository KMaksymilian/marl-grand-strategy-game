//! Trade phases of the settlement turn: reservation, caravan funding,
//! market trading with visitors and collection of returns.
use super::Settlement;
use super::turn_report::{
    DispatchedCaravan, MarketTrade, MaterialTotals, ReturnedCaravan, TradeTurnReport,
};
use crate::economy::ResourceType;
use crate::trade::allocation::TradeAllocationEngine;
use crate::trade::book::TradeBook;
use crate::trade::caravan::{Caravan, CaravanCargo, CaravanState};
use std::collections::HashMap;

fn totals<'a>(goods: impl IntoIterator<Item = (&'a ResourceType, &'a u32)>) -> MaterialTotals {
    goods
        .into_iter()
        .filter(|(resource, amount)| **resource != ResourceType::Labour && **amount > 0)
        .map(|(&resource, &amount)| (resource, u64::from(amount)))
        .collect()
}

impl Settlement {
    /// Moves stock and gold requested by the published offers into the market book.
    pub(super) fn reserve_market(&mut self, report: &mut TradeTurnReport) {
        self.book.replace_offers(self.trade.market_offers.clone());
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

    /// Funds queued caravan orders in queue order; orders with nothing funded are skipped.
    pub(super) fn fund_caravan_orders(&mut self, report: &mut TradeTurnReport) {
        for order in std::mem::take(&mut self.trade.orders) {
            let mut book = TradeBook::with_offers(order.offers);
            let demands = book.calculate_demand();
            TradeAllocationEngine::execute(
                &mut book,
                &mut self.gold_inventory,
                &mut self.inventory_manager.main,
                demands,
            );
            let cargo = totals(&book.goods_for_sale());
            if cargo.is_empty() && book.gold() == 0 {
                report.unfunded_caravan_orders.push(order.receiver_id);
                continue;
            }
            report.caravans_dispatched.push(DispatchedCaravan {
                receiver_id: order.receiver_id,
                cargo,
                gold: u64::from(book.gold()),
            });
            self.trade.dispatched.push(CaravanCargo {
                sender_id: self.id,
                receiver_id: order.receiver_id,
                book,
            });
        }
    }

    /// Trades the market book with caravans that arrived here; the book is shared equally.
    pub(super) fn trade_with_visitors(
        &mut self,
        visitors: &mut [&mut Caravan],
        report: &mut TradeTurnReport,
    ) {
        let mut guests: Vec<&mut Caravan> = visitors
            .iter_mut()
            .filter(|caravan| {
                caravan.state() == CaravanState::Trading && caravan.receiver_id == self.id as usize
            })
            .map(|caravan| &mut **caravan)
            .collect();
        if guests.is_empty() {
            return;
        }
        let log = {
            let mut books: Vec<&mut TradeBook> = guests
                .iter_mut()
                .map(|caravan| caravan.book_mut())
                .collect();
            self.book.trade_with_visitors(&mut books)
        };
        for (caravan, executions) in guests.into_iter().zip(log) {
            let caravan_sender_id = caravan.sender_id as u32;
            report
                .market_trades
                .extend(executions.iter().map(|&execution| MarketTrade {
                    caravan_sender_id,
                    execution,
                }));
            caravan.finish_trading(executions);
        }
    }

    /// Drains the market book and returned caravans. Gold goes straight to the treasury;
    /// goods are returned as one batch that the turn stores together with production.
    pub(super) fn collect_trade_returns(
        &mut self,
        report: &mut TradeTurnReport,
    ) -> HashMap<ResourceType, u64> {
        let mut batch: HashMap<ResourceType, u64> = HashMap::new();
        let mut add = |goods: &HashMap<ResourceType, u32>| {
            for (&resource, &amount) in goods {
                if resource != ResourceType::Labour && amount > 0 {
                    *batch.entry(resource).or_default() += u64::from(amount);
                }
            }
        };

        report.market_bought = totals(&self.book.bought_goods());
        report.market_unsold = totals(&self.book.goods_for_sale());
        let (goods, gold) = self.book.return_resources();
        add(&goods);
        self.gold_inventory.add(gold);
        report.gold_received += u64::from(gold);

        for mut caravan in std::mem::take(&mut self.trade.returning) {
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

        report.received = batch
            .iter()
            .map(|(&resource, &amount)| (resource, amount))
            .collect();
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::TradeCommandError;
    use crate::trade::offer::TradeOffer;
    use crate::world::map::Map;
    use crate::world::pathfinding::path_finder::PathFinder;
    use crate::world::terrain::area::Area;
    use std::sync::Arc;

    fn turn(settlement: &mut Settlement) -> crate::settlement::turn_report::TurnReport {
        let map = Map::new(4, 1, vec![Area::new(0.5, 0.5); 4], vec![0; 4]);
        settlement.process_turn(&map, &mut PathFinder::new(4, 1), &mut [])
    }

    #[test]
    fn unsold_market_stock_and_gold_return_each_turn_and_stay_published() {
        let mut settlement = Settlement::new(1, 50, 0, Arc::default());
        settlement.deposit(ResourceType::Wood, 5);
        settlement.deposit_gold(20);
        settlement
            .publish_trade_offers(HashMap::from([
                (ResourceType::Wood, TradeOffer::sell(8, 2)),
                (ResourceType::Stone, TradeOffer::buy(4, 3)),
            ]))
            .unwrap();
        for _ in 0..2 {
            let report = turn(&mut settlement);
            assert_eq!(report.trade.market_reserved[&ResourceType::Wood], 5);
            assert_eq!(report.trade.market_gold_reserved, 12);
            assert_eq!(report.trade.market_unsold[&ResourceType::Wood], 5);
            assert_eq!(report.trade.gold_received, 12);
            assert!(report.inventory.losses.is_empty());
            assert!(report.inventory.is_balanced());
            assert_eq!(settlement.inventory().amount(ResourceType::Wood), 5);
            assert_eq!(settlement.gold(), 20);
        }
    }

    #[test]
    fn caravan_orders_are_funded_after_the_market_in_queue_order() {
        let mut settlement = Settlement::new(1, 50, 0, Arc::default());
        settlement.deposit(ResourceType::Grain, 10);
        settlement
            .publish_trade_offers(HashMap::from([(
                ResourceType::Grain,
                TradeOffer::sell(6, 1),
            )]))
            .unwrap();
        for receiver in [2, 3] {
            settlement
                .order_caravan(
                    receiver,
                    HashMap::from([(ResourceType::Grain, TradeOffer::sell(3, 1))]),
                )
                .unwrap();
        }
        let report = turn(&mut settlement);
        let dispatched = &report.trade.caravans_dispatched;
        assert_eq!(dispatched.len(), 2);
        assert_eq!(dispatched[0].cargo[&ResourceType::Grain], 3);
        assert_eq!(dispatched[1].cargo[&ResourceType::Grain], 1);
        assert_eq!(report.inventory.trade_reserved[&ResourceType::Grain], 10);
        assert!(report.inventory.is_balanced());
        assert_eq!(settlement.inventory().amount(ResourceType::Grain), 6);
        assert_eq!(settlement.take_dispatched_caravans().len(), 2);
        assert!(settlement.caravan_orders().is_empty());
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
        let mut settlement = Settlement::new(1, 50, 0, Arc::default());
        settlement.deposit_gold(10);
        settlement
            .publish_trade_offers(HashMap::from([(
                ResourceType::Wood,
                partially_bought(3, 1, 2),
            )]))
            .unwrap();
        for _ in 0..3 {
            let report = turn(&mut settlement);
            assert!(report.trade.received.is_empty());
            assert!(report.inventory.is_balanced());
            assert_eq!(settlement.inventory().amount(ResourceType::Wood), 0);
            assert_eq!(settlement.gold(), 10);
        }
    }

    #[test]
    fn caravan_order_does_not_carry_earlier_purchases() {
        let mut settlement = Settlement::new(1, 50, 0, Arc::default());
        settlement.deposit_gold(10);
        settlement
            .order_caravan(
                2,
                HashMap::from([(ResourceType::Wood, partially_bought(3, 1, 2))]),
            )
            .unwrap();
        turn(&mut settlement);
        let cargo = settlement.take_dispatched_caravans().pop().unwrap();
        assert!(cargo.book.bought_goods().is_empty());
    }

    #[test]
    fn invalid_trade_commands_are_rejected() {
        let mut settlement = Settlement::new(1, 50, 0, Arc::default());
        let labour = HashMap::from([(ResourceType::Labour, TradeOffer::sell(1, 1))]);
        assert_eq!(
            settlement.publish_trade_offers(labour.clone()),
            Err(TradeCommandError::LabourIsNotTradable)
        );
        assert_eq!(
            settlement.order_caravan(2, labour),
            Err(TradeCommandError::LabourIsNotTradable)
        );
        let wood = HashMap::from([(ResourceType::Wood, TradeOffer::buy(1, 1))]);
        assert_eq!(
            settlement.order_caravan(1, wood),
            Err(TradeCommandError::TradeWithSelf)
        );
        assert_eq!(
            settlement.order_caravan(
                2,
                HashMap::from([(ResourceType::Wood, TradeOffer::buy(0, 1))])
            ),
            Err(TradeCommandError::EmptyOrder)
        );
        assert!(settlement.caravan_orders().is_empty());
    }
}
