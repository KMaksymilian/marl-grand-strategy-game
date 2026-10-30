//! Records of executed bilateral trades.
use crate::economy::ResourceType;

/// Direction of a trade, seen from the book that records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TradeSide {
    Bought,
    Sold,
}

impl TradeSide {
    pub fn opposite(self) -> Self {
        match self {
            TradeSide::Bought => TradeSide::Sold,
            TradeSide::Sold => TradeSide::Bought,
        }
    }
}

/// One resource exchanged between two books at a single unit price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeExecution {
    pub resource: ResourceType,
    pub units: u32,
    pub unit_price: u32,
    pub side: TradeSide,
}

impl TradeExecution {
    pub fn total_price(&self) -> u64 {
        u64::from(self.units) * u64::from(self.unit_price)
    }

    /// The same trade seen from the other participant.
    pub fn mirrored(self) -> Self {
        Self {
            side: self.side.opposite(),
            ..self
        }
    }
}
