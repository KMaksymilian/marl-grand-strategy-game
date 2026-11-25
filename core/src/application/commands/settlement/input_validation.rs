//! Checks submitted data without accessing simulation state.
use super::dispatch::trade_error;
use super::{ActionError, OfferSpec, SettlementAction};
use crate::domain::economy::ResourceType;
use crate::domain::settlement::TradeCommandError;
use crate::domain::trade::offer::TradeOffer;
use std::collections::{HashMap, hash_map::Entry};

/// Validates constraints decidable from the command's fields alone.
/// Unsigned worker counts, units and prices reject negatives during JSON parsing.
pub fn validate_input(action: &SettlementAction) -> Result<(), ActionError> {
    match action {
        SettlementAction::Construct { position, .. } if position.x < 0 || position.y < 0 => {
            Err(ActionError::InvalidPosition {
                position: *position,
            })
        }
        SettlementAction::PublishMarketOffers { offers } => validate_offers(offers),
        SettlementAction::OrderCaravan { offers, .. } if offers.is_empty() => {
            Err(ActionError::EmptyOrder)
        }
        SettlementAction::OrderCaravan { offers, .. } => validate_offers(offers),
        _ => Ok(()),
    }
}

fn validate_offers(offers: &[OfferSpec]) -> Result<(), ActionError> {
    TradeCommandError::validate_offer_shapes(&resolve_offers(offers)?).map_err(trade_error)
}

pub(super) fn resolve_offers(
    offers: &[OfferSpec],
) -> Result<HashMap<ResourceType, TradeOffer>, ActionError> {
    let mut resolved = HashMap::with_capacity(offers.len());
    for offer in offers {
        match resolved.entry(offer.resource.clone()) {
            Entry::Occupied(_) => {
                return Err(ActionError::DuplicateOffer {
                    resource: offer.resource.clone(),
                });
            }
            Entry::Vacant(entry) => {
                entry.insert(offer.to_offer());
            }
        }
    }
    Ok(resolved)
}
