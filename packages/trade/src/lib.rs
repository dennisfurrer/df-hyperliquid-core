//! Hyperliquid order primitives shared by every market kind: order, cancel, modify, TIF rules (market orders are FrontendMarket, never Gtc), tick and lot rounding.
//!
//! Every action here is an L1 action ([`L1Action`]): an approved agent key may
//! sign it for its master. A market order is [`Order::market`], which is always
//! `FrontendMarket`: a `Gtc` at the same price is a different order, one that
//! rests on the book when it does not fill.
//!
//! Prices and sizes go on the wire exactly as given. [`Grid`] puts them on an
//! asset's grid first, the way the venue's own clients do: a price never
//! rounds against the trader, a size rounds toward zero.
#![deny(clippy::float_arithmetic)]

pub mod cancel;
pub mod error;
pub mod grid;
pub mod leverage;
pub mod modify;
pub mod order;
pub mod twap;

pub use cancel::{Cancel, CancelByCloid};
pub use error::TradeError;
pub use grid::{Grid, MAX_SLIPPAGE, MINIMUM_ORDER_VALUE, PERP_PRICE_PLACES, SIGNIFICANT_FIGURES, SPOT_PRICE_PLACES};
pub use hl_core::{Asset, AssetKind, BuilderFee, L1Action};
pub use leverage::{MarginMode, UpdateIsolatedMargin, UpdateLeverage};
pub use modify::{BatchModify, Modify, OrderRef};
pub use order::{Builder, Cloid, Grouping, Order, OrderType, OverCap, PlaceOrder, Side, Tif, Tpsl};
pub use twap::{Twap, TwapCancel};
