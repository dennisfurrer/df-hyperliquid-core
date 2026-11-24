//! Received frames (`{"channel":..,"data":..}`) read into a type per channel ([`decode`]): live's
//! own frame types, or the [`crate::account`] and [`crate::markets`] types where the socket sends
//! the shape `/info` answers. Numbers are exact decimals, never floats. An unknown channel is
//! [`Frame::Other`], kept as written; a frame that does not read is a [`BadFrame`], never a panic.

pub mod control;
pub mod read;
pub mod reply;

pub use control::{Ack, Method, Refusal};
pub use read::{BadFrame, decode};
pub use reply::{Answer, Reply};

use crate::account::ActiveAssetData;
use crate::live::{
    AllDexsState, Bbo, DexState, HistoricalOrders, L2, Ledger, Mids, OpenOrders, OrderUpdate, SpotState, TwapHistory,
    TwapSliceFills, TwapStates, UserEvent, UserFills, UserFundings, WebData3,
};
use crate::markets::context::{PerpCtx, SpotCtx};
use crate::markets::{Book, Candle, Trade};
use crate::utils::Json;

/// One frame, by channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// `pong`: the answer to a ping.
    Pong,
    /// `subscriptionResponse`: the venue took a subscribe or an unsubscribe.
    Ack(Ack),
    /// `error`: the venue refused something sent, in its own words.
    Error(Refusal),
    /// `post`: the answer to a post, by its id.
    Post(Reply),
    /// `allMids`: every coin's mid on one perp dex.
    AllMids(Mids),
    /// `l2Book`.
    Book(Book),
    /// `trades`: new prints on one coin.
    Trades(Vec<Trade>),
    /// `bbo`: the best bid and offer of one coin.
    Bbo(Bbo),
    /// `candle`: one candle, sent as it changes.
    Candle(Candle),
    /// `activeAssetCtx`: a perp's context.
    AssetCtx {
        /// The perp's coin.
        coin: String,
        /// The perp's context.
        ctx: PerpCtx,
    },
    /// `activeSpotAssetCtx`: what `activeAssetCtx` sends for a spot pair or an outcome side.
    SpotAssetCtx {
        /// The spot pair's or outcome side's coin.
        coin: String,
        /// The pair's or side's context.
        ctx: SpotCtx,
    },
    /// `activeAssetData`.
    AssetData(ActiveAssetData),
    /// `userFills`: the account's fills.
    UserFills(UserFills),
    /// `orderUpdates`: status changes of the account's orders.
    OrderUpdates(Vec<OrderUpdate>),
    /// `user`: what `userEvents` sends.
    UserEvent(UserEvent),
    /// `userFundings`: the account's funding payments.
    UserFundings(UserFundings),
    /// `userNonFundingLedgerUpdates`.
    Ledger(Ledger),
    /// `clearinghouseState`: the account's clearinghouse on one dex.
    ClearinghouseState(DexState),
    /// `allDexsClearinghouseState`: the account's clearinghouse on every dex.
    AllDexsClearinghouseState(AllDexsState),
    /// `openOrders`: the account's resting orders on one dex.
    OpenOrders(OpenOrders),
    /// `spotState`: the account's spot balances.
    SpotState(SpotState),
    /// `twapStates`: the account's running TWAPs on one dex.
    TwapStates(TwapStates),
    /// `userTwapSliceFills`.
    TwapSliceFills(TwapSliceFills),
    /// `userTwapHistory`.
    TwapHistory(TwapHistory),
    /// `notification`: a short text for the account, as sent.
    Notification(String),
    /// `webData3`: the account as Hyperliquid's own frontend reads it.
    WebData3(WebData3),
    /// Every perp dex's contexts, by dex name, the first `""`.
    AllDexsAssetCtxs(Vec<(String, Vec<PerpCtx>)>),
    /// `assetCtxs`: one perp dex's contexts.
    AssetCtxs {
        /// The dex, `""` for the first.
        dex: String,
        /// The contexts, in the dex's universe order.
        ctxs: Vec<PerpCtx>,
    },
    /// `spotAssetCtxs`: every spot pair's and outcome side's context.
    SpotAssetCtxs(Vec<SpotCtx>),
    /// What changed in `outcomeMeta`, kept as sent: `[{"outcomeSettled":23205}]`. Its arrival
    /// is the news; read `outcomeMeta` again for the whole.
    OutcomeMetaUpdates(Json),
    /// `fastAssetCtxs`: base64 of raw DEFLATE (RFC 1951) over JSON, kept as sent. Inflating it
    /// needs a decompressor this crate does not carry.
    FastAssetCtxs(String),
    /// `userHistoricalOrders`.
    HistoricalOrders(HistoricalOrders),
    /// `l2`: the book's snapshot, or a diff to it.
    L2(L2),
    /// `pac`: every perp dex's contexts as `[dex, ctxs]` pairs, the first dex `""`, each `ctxs` in
    /// the dex's universe order. After the snapshot, a context holds only the fields that changed,
    /// or is `{}`. Base64 of raw DEFLATE (RFC 1951) over JSON, kept as sent.
    Pac(String),
    /// `sac`: every spot pair's and outcome side's context by coin; after the snapshot, only the
    /// coins and fields that changed. Base64 of raw DEFLATE (RFC 1951) over JSON, kept as sent.
    Sac(String),
    /// A channel this build does not read: its name, and its data as written (empty when the
    /// frame has none).
    Other {
        /// The channel's name.
        channel: String,
        /// The frame's `data` as written JSON; empty when the frame has none.
        data: String,
    },
}
