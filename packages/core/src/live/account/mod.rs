//! The websocket's own frames about one account: fills, order updates, events, funding, the
//! ledger, each dex's clearinghouse, spot balances, TWAPs and `webData3`. Rows the `/info` reads
//! answer too (a fill, an order, a clearinghouse) are the [`crate::account`] types. Amounts are
//! exact [`Dec`](crate::utils::Dec); the venue writes them as strings. Frames that open with
//! history say so in `is_snapshot`.

pub mod events;
pub mod history;
pub mod orders;
pub mod state;
pub mod twap;
pub mod web_data;

pub use events::{Cancel, Liquidation, UserEvent};
pub use history::{Funding, Ledger, UserFills, UserFundings};
pub use orders::{HistoricalOrders, OpenOrders, OrderUpdate};
pub use state::{AllDexsState, DexState, SpotState};
pub use twap::{TwapEvent, TwapHistory, TwapSliceFills, TwapState, TwapStates, TwapStatus};
pub use web_data::{LeadingVault, PerpDexState, UserState, WebData3};
