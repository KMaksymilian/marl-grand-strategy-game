//! Fair sharing of one market book between several visiting books.
//!
//! Each round splits every contested offer, and the market's gold, equally among
//! visitors that can still trade on it. Each visitor trades with its own share and
//! the shares are merged back. Rounds repeat until nobody can trade, so stock left
//! unused by one visitor is offered again to the others. Integer remainders rotate
//! between rounds. Units that cannot be split usefully (for example gold shares
//! below one unit price) are offered at the end to visitors in their given order.
use super::TradeBook;
use crate::economy::ResourceType;
use crate::trade::execution::TradeExecution;
use crate::trade::offer::TradeOffer;
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound on sharing rounds; every productive round trades at least one unit.
const MAX_SHARING_ROUNDS: usize = 64;

#[derive(Default)]
struct Interest {
    /// Visitors able to trade each resource with the market.
    units: BTreeMap<ResourceType, Vec<usize>>,
    /// Visitors selling something the market buys; they share its gold.
    gold: BTreeSet<usize>,
}

impl TradeBook {
    /// Trades this book with every visitor, sharing contested offers equally.
    /// Returns executions per visitor (same order), seen from this book.
    pub fn trade_with_visitors(
        &mut self,
        visitors: &mut [&mut TradeBook],
    ) -> Vec<Vec<TradeExecution>> {
        let mut log: Vec<Vec<TradeExecution>> = vec![Vec::new(); visitors.len()];
        if let [visitor] = visitors {
            log[0] = self.trade(visitor);
            return log;
        }

        for round in 0..MAX_SHARING_ROUNDS {
            let interest = self.visitor_interest(visitors);
            if interest.units.is_empty() {
                return log;
            }
            let (gold_shares, unit_shares) = self.equal_shares(&interest, visitors.len(), round);
            let mut progressed = false;
            for (index, visitor) in visitors.iter_mut().enumerate() {
                if unit_shares[index].is_empty() {
                    continue;
                }
                let mut share = self.take_share(gold_shares[index], &unit_shares[index]);
                let executed = share.trade(visitor);
                self.absorb(share);
                progressed |= !executed.is_empty();
                log[index].extend(executed);
            }
            if !progressed {
                break;
            }
        }

        for (index, visitor) in visitors.iter_mut().enumerate() {
            log[index].extend(self.trade(visitor));
        }
        log
    }

    fn visitor_interest(&self, visitors: &[&mut TradeBook]) -> Interest {
        let mut interest = Interest::default();
        let offers: BTreeMap<_, _> = self.offers.iter().collect();
        for (&resource, offer) in offers {
            if offer.remaining_units() == 0 {
                continue;
            }
            for (index, visitor) in visitors.iter().enumerate() {
                let Some(other) = visitor.offers.get(&resource) else {
                    continue;
                };
                if other.remaining_units() == 0 {
                    continue;
                }
                let (price, buyer_gold) = match (offer, other) {
                    (TradeOffer::Sell(sell), TradeOffer::Buy(buy)) => (
                        Self::price(
                            sell.get_minimal_price_per_unit(),
                            buy.get_maximum_price_per_unit(),
                        ),
                        visitor.gold,
                    ),
                    (TradeOffer::Buy(buy), TradeOffer::Sell(sell)) => (
                        Self::price(
                            sell.get_minimal_price_per_unit(),
                            buy.get_maximum_price_per_unit(),
                        ),
                        self.gold,
                    ),
                    _ => continue,
                };
                if let Some(price) = price
                    && buyer_gold >= price
                {
                    interest.units.entry(resource).or_default().push(index);
                    if matches!(offer, TradeOffer::Buy(_)) {
                        interest.gold.insert(index);
                    }
                }
            }
        }
        interest
    }

    /// Midpoint unit price, or `None` when the offers cannot match.
    fn price(minimum: u32, maximum: u32) -> Option<u32> {
        let price = (minimum + maximum) / 2;
        (minimum <= maximum && price > 0).then_some(price)
    }

    fn equal_shares(
        &self,
        interest: &Interest,
        visitor_count: usize,
        round: usize,
    ) -> (Vec<u32>, Vec<BTreeMap<ResourceType, u32>>) {
        let mut units = vec![BTreeMap::new(); visitor_count];
        for (resource, visitors) in &interest.units {
            let total = self.offers[resource].remaining_units();
            for (&visitor, share) in visitors
                .iter()
                .zip(Self::split(total, visitors.len(), round))
            {
                units[visitor].insert(*resource, share);
            }
        }
        let mut gold = vec![0; visitor_count];
        let recipients: Vec<_> = interest.gold.iter().copied().collect();
        for (&visitor, share) in
            recipients
                .iter()
                .zip(Self::split(self.gold, recipients.len(), round))
        {
            gold[visitor] = share;
        }
        (gold, units)
    }

    /// Splits `total` into `parts` near-equal values; remainders start at `round`.
    fn split(total: u32, parts: usize, round: usize) -> Vec<u32> {
        if parts == 0 {
            return Vec::new();
        }
        let parts_u32 = u32::try_from(parts).unwrap_or(u32::MAX);
        let base = total / parts_u32;
        let remainder = (total % parts_u32) as usize;
        (0..parts)
            .map(|index| base + u32::from((index + parts - round % parts) % parts < remainder))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::execution::TradeSide;
    use std::collections::HashMap;

    fn units(executions: &[TradeExecution]) -> u32 {
        executions.iter().map(|execution| execution.units).sum()
    }

    fn book(gold: u32, offers: &[(ResourceType, TradeOffer)]) -> TradeBook {
        TradeBook::new(gold, offers.iter().cloned().collect::<HashMap<_, _>>())
    }

    #[test]
    fn equal_buyers_split_scarce_stock_equally() {
        let mut market = book(0, &[(ResourceType::Wood, TradeOffer::sell(10, 2))]);
        let mut first = book(100, &[(ResourceType::Wood, TradeOffer::buy(10, 2))]);
        let mut second = book(100, &[(ResourceType::Wood, TradeOffer::buy(10, 2))]);
        let log = market.trade_with_visitors(&mut [&mut first, &mut second]);
        assert_eq!(units(&log[0]), 5);
        assert_eq!(units(&log[1]), 5);
        assert!(log.iter().flatten().all(|e| e.side == TradeSide::Sold));
        assert_eq!(market.gold(), 20);
        assert_eq!(first.bought_goods()[&ResourceType::Wood], 5);
    }

    #[test]
    fn stock_unused_by_a_small_buyer_goes_to_the_others() {
        let mut market = book(0, &[(ResourceType::Wood, TradeOffer::sell(10, 2))]);
        let mut small = book(100, &[(ResourceType::Wood, TradeOffer::buy(1, 2))]);
        let mut large = book(100, &[(ResourceType::Wood, TradeOffer::buy(100, 2))]);
        let log = market.trade_with_visitors(&mut [&mut small, &mut large]);
        assert_eq!(units(&log[0]), 1);
        assert_eq!(units(&log[1]), 9);
        assert!(market.goods_for_sale().is_empty());
    }

    #[test]
    fn market_gold_is_shared_between_sellers() {
        let mut market = book(40, &[(ResourceType::Stone, TradeOffer::buy(100, 2))]);
        let mut first = book(0, &[(ResourceType::Stone, TradeOffer::sell(50, 2))]);
        let mut second = book(0, &[(ResourceType::Stone, TradeOffer::sell(50, 2))]);
        let log = market.trade_with_visitors(&mut [&mut first, &mut second]);
        assert_eq!(units(&log[0]), 10);
        assert_eq!(units(&log[1]), 10);
        assert_eq!(first.gold() + second.gold(), 40);
        assert_eq!(market.gold(), 0);
        assert_eq!(market.bought_goods()[&ResourceType::Stone], 20);
    }

    #[test]
    fn indivisible_leftovers_are_still_traded() {
        // 3 gold at price 2 cannot be split usefully between two sellers.
        let mut market = book(3, &[(ResourceType::Grain, TradeOffer::buy(5, 2))]);
        let mut first = book(0, &[(ResourceType::Grain, TradeOffer::sell(5, 2))]);
        let mut second = book(0, &[(ResourceType::Grain, TradeOffer::sell(5, 2))]);
        let log = market.trade_with_visitors(&mut [&mut first, &mut second]);
        assert_eq!(units(&log[0]) + units(&log[1]), 1);
        assert_eq!(market.gold(), 1);
    }

    #[test]
    fn split_rotates_remainders() {
        assert_eq!(TradeBook::split(7, 3, 0), vec![3, 2, 2]);
        assert_eq!(TradeBook::split(7, 3, 1), vec![2, 3, 2]);
        assert_eq!(TradeBook::split(7, 3, 2), vec![2, 2, 3]);
        assert_eq!(TradeBook::split(8, 3, 3), vec![3, 3, 2]);
    }
}
