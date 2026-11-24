//! Offer state, reservations and resource collection.
mod market;
mod matching;

use crate::{economy::ResourceType, trade::offer::TradeOffer};
use std::collections::{BTreeMap, HashMap};

/// Funded offers available for bilateral trade.
#[derive(Debug)]
pub struct TradeBook {
    gold: u32,
    offers: HashMap<ResourceType, TradeOffer>,
}

impl TradeBook {
    pub fn new(gold: u32, offers: HashMap<ResourceType, TradeOffer>) -> Self {
        TradeBook { gold, offers }
    }

    /// Unfunded book; reserve its demand before trading.
    pub fn with_offers(offers: HashMap<ResourceType, TradeOffer>) -> Self {
        Self::new(0, offers)
    }

    pub fn gold(&self) -> u32 {
        self.gold
    }

    pub fn offers(&self) -> &HashMap<ResourceType, TradeOffer> {
        &self.offers
    }

    /// Replaces all offers and returns the previous ones. Reserved gold is kept.
    pub fn replace_offers(
        &mut self,
        offers: HashMap<ResourceType, TradeOffer>,
    ) -> HashMap<ResourceType, TradeOffer> {
        std::mem::replace(&mut self.offers, offers)
    }

    pub fn add_gold_amount(&mut self, gold_amount: u32) {
        self.gold += gold_amount;
    }

    pub fn update_sell_resource_amount(&mut self, res_type: ResourceType, avaiable_units: u32) {
        if let Some(offer) = self.offers.get_mut(&res_type)
            && let TradeOffer::Sell(sell_offer) = offer
        {
            sell_offer.update_sell_resource_amount(avaiable_units);
        }
    }

    pub fn calculate_demand(&self) -> (HashMap<ResourceType, u32>, u32) {
        let (goods, gold) = Self::funding_demand(&self.offers);
        (
            goods.into_iter().collect(),
            u32::try_from(gold).unwrap_or(u32::MAX),
        )
    }

    /// Exact sell quantities and full purchase budget before checking the funding limit.
    pub(crate) fn funding_demand(
        offers: &HashMap<ResourceType, TradeOffer>,
    ) -> (BTreeMap<ResourceType, u32>, u64) {
        let mut goods = BTreeMap::new();
        let mut gold = 0_u64;
        for (resource, offer) in offers {
            match offer {
                TradeOffer::Sell(sell) => {
                    goods.insert(resource.clone(), sell.get_amount_to_sell());
                }
                TradeOffer::Buy(buy) => {
                    let budget = u64::from(buy.get_remaining_amount_to_buy())
                        * u64::from(buy.get_maximum_price_per_unit());
                    gold = gold.saturating_add(budget);
                }
            }
        }
        (goods, gold)
    }

    /// Goods still held for sale (nonzero entries only).
    pub fn goods_for_sale(&self) -> BTreeMap<ResourceType, u32> {
        self.offers
            .iter()
            .filter_map(|(resource, offer)| match offer {
                TradeOffer::Sell(sell) if sell.get_amount_to_sell() > 0 => {
                    Some((resource.clone(), sell.get_amount_to_sell()))
                }
                _ => None,
            })
            .collect()
    }

    /// Goods already purchased and waiting to be collected (nonzero entries only).
    pub fn bought_goods(&self) -> BTreeMap<ResourceType, u32> {
        self.offers
            .iter()
            .filter_map(|(resource, offer)| match offer {
                TradeOffer::Buy(buy) if buy.get_already_bought_units() > 0 => {
                    Some((resource.clone(), buy.get_already_bought_units()))
                }
                _ => None,
            })
            .collect()
    }

    pub fn return_resources(&mut self) -> (HashMap<ResourceType, u32>, u32) {
        let mut returned_resources: HashMap<ResourceType, u32> = HashMap::new();
        let returned_gold: u32 = std::mem::take(&mut self.gold);

        for (resource_type, offer) in &mut self.offers {
            match offer {
                TradeOffer::Buy(buy_offer) => {
                    let bought: u32 = buy_offer.consume_bought_units();
                    *returned_resources.entry(resource_type.clone()).or_insert(0) += bought;
                }
                TradeOffer::Sell(sell_offer) => {
                    let remained: u32 = sell_offer.consume_avaiable_units();
                    *returned_resources.entry(resource_type.clone()).or_insert(0) += remained;
                }
            }
        }

        (returned_resources, returned_gold)
    }

    /// Moves `gold` and the given open units into a separate book with the same prices.
    pub(crate) fn take_share(
        &mut self,
        gold: u32,
        units: &BTreeMap<ResourceType, u32>,
    ) -> TradeBook {
        let gold = gold.min(self.gold);
        self.gold -= gold;
        let offers = units
            .iter()
            .filter_map(|(resource, &amount)| {
                let offer = self.offers.get_mut(resource)?;
                Some((resource.clone(), offer.take_share(amount)))
            })
            .collect();
        TradeBook::new(gold, offers)
    }

    /// Merges a book created by `take_share` back, including its trade results.
    pub(crate) fn absorb(&mut self, share: TradeBook) {
        self.gold += share.gold;
        for (resource, offer) in share.offers {
            match self.offers.get_mut(&resource) {
                Some(existing) => existing.absorb(offer),
                None => {
                    self.offers.insert(resource, offer);
                }
            }
        }
    }
}

impl Default for TradeBook {
    fn default() -> Self {
        let gold: u32 = 0;
        let offers: HashMap<ResourceType, TradeOffer> = HashMap::new();
        Self::new(gold, offers)
    }
}
