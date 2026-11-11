//! Checks submitted data without accessing simulation state.
use super::{ActionError, OfferSpec, SettlementAction};
use crate::domain::economy::ResourceType;
use std::collections::HashSet;

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
    let mut resources = HashSet::new();
    for offer in offers {
        if offer.resource == ResourceType::Labour {
            return Err(ActionError::LabourIsNotTradable);
        }
        if offer.units == 0 {
            return Err(ActionError::ZeroUnits {
                resource: offer.resource,
            });
        }
        if !resources.insert(offer.resource) {
            return Err(ActionError::DuplicateOffer {
                resource: offer.resource,
            });
        }
    }
    Ok(())
}
