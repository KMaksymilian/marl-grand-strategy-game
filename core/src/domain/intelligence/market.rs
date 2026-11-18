use super::{Information, InformationSnapshot, ObservationStrategy};
use crate::domain::ids::SettlementId;
use crate::domain::trade::{book::TradeBook, offer::TradeOffer};

/// Collects the open offers visible at a visited settlement's market.
pub struct MarketObservation;

pub struct MarketObservationSource<'a> {
    pub market: SettlementId,
    pub province_id: usize,
    pub book: &'a TradeBook,
}

impl ObservationStrategy<MarketObservationSource<'_>> for MarketObservation {
    fn observe(
        &self,
        observed_turn: u64,
        source: MarketObservationSource<'_>,
    ) -> InformationSnapshot {
        let offers = source
            .book
            .offers()
            .iter()
            .filter_map(|(resource, offer)| {
                let remaining = offer.remaining_units();
                let (side, unit_price_limit, units) = match offer {
                    TradeOffer::Buy(buy) => {
                        let limit = buy.get_maximum_price_per_unit();
                        let affordable = source.book.gold().checked_div(limit).unwrap_or(0);
                        (MarketOfferSide::Buy, limit, remaining.min(affordable))
                    }
                    TradeOffer::Sell(sell) => (
                        MarketOfferSide::Sell,
                        sell.get_minimal_price_per_unit(),
                        remaining,
                    ),
                };
                (units > 0).then_some((
                    resource.clone(),
                    MarketOffer {
                        side,
                        units,
                        unit_price_limit,
                    },
                ))
            })
            .collect();

        InformationSnapshot::new(
            observed_turn,
            Information::ForeignMarket {
                market: source.market,
                province_id: source.province_id,
                offers,
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketOffer {
    pub side: MarketOfferSide,
    /// For buys, an estimate at the price limit using the shared market budget.
    /// Buy quantities cannot be added to infer total purchasing capacity.
    pub units: u32,
    /// Maximum price for a buy offer or minimum price for a sell offer.
    pub unit_price_limit: u32,
}

/// Direction from the visited market's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketOfferSide {
    Buy,
    Sell,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    #[test]
    fn captures_open_offers_without_following_later_market_changes() {
        let market = SettlementId(4);
        let mut book = TradeBook::new(
            10,
            [
                (
                    crate::test_support::material("wood"),
                    TradeOffer::sell(6, 11),
                ),
                (
                    crate::test_support::material("grain"),
                    TradeOffer::buy(9, 3),
                ),
                (
                    crate::test_support::material("stone"),
                    TradeOffer::sell(0, 7),
                ),
            ]
            .into(),
        );

        let snapshot = MarketObservation.observe(
            12,
            MarketObservationSource {
                market,
                province_id: 20,
                book: &book,
            },
        );
        book.replace_offers(Default::default());

        assert_eq!(snapshot.observed_turn(), 12);
        assert_eq!(
            snapshot.information(),
            &Information::ForeignMarket {
                market,
                province_id: 20,
                offers: BTreeMap::from([
                    (
                        crate::test_support::material("wood"),
                        MarketOffer {
                            side: MarketOfferSide::Sell,
                            units: 6,
                            unit_price_limit: 11,
                        },
                    ),
                    (
                        crate::test_support::material("grain"),
                        MarketOffer {
                            side: MarketOfferSide::Buy,
                            units: 3,
                            unit_price_limit: 3,
                        },
                    ),
                ]),
            }
        );
        assert_eq!(
            MarketObservation.observe(
                13,
                MarketObservationSource {
                    market,
                    province_id: 20,
                    book: &book,
                },
            ),
            InformationSnapshot::new(
                13,
                Information::ForeignMarket {
                    market,
                    province_id: 20,
                    offers: BTreeMap::new(),
                },
            )
        );
    }
}
