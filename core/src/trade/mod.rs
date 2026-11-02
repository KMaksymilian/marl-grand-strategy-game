//! Trade offers, funded order books, matching and caravan transport.
pub mod allocation;
pub mod book;
pub mod caravan;
pub mod execution;
pub mod network;
pub mod offer;

pub use book::TradeBook;
pub use execution::{TradeExecution, TradeSide};
pub use offer::TradeOffer;
