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
    AllMids(Mids),
    /// `l2Book`.
    Book(Book),
    Trades(Vec<Trade>),
    Bbo(Bbo),
    Candle(Candle),
    /// `activeAssetCtx`: a perp's context.
    AssetCtx {
        coin: String,
        ctx: PerpCtx,
    },
    /// `activeSpotAssetCtx`: what `activeAssetCtx` sends for a spot pair or an outcome side.
    SpotAssetCtx {
        coin: String,
        ctx: SpotCtx,
    },
    /// `activeAssetData`.
    AssetData(ActiveAssetData),
    UserFills(UserFills),
    OrderUpdates(Vec<OrderUpdate>),
    /// `user`: what `userEvents` sends.
    UserEvent(UserEvent),
    UserFundings(UserFundings),
    /// `userNonFundingLedgerUpdates`.
    Ledger(Ledger),
    ClearinghouseState(DexState),
    AllDexsClearinghouseState(AllDexsState),
    OpenOrders(OpenOrders),
    SpotState(SpotState),
    TwapStates(TwapStates),
    /// `userTwapSliceFills`.
    TwapSliceFills(TwapSliceFills),
    /// `userTwapHistory`.
    TwapHistory(TwapHistory),
    Notification(String),
    WebData3(WebData3),
    /// Every perp dex's contexts, by dex name, the first `""`.
    AllDexsAssetCtxs(Vec<(String, Vec<PerpCtx>)>),
    AssetCtxs {
        dex: String,
        ctxs: Vec<PerpCtx>,
    },
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
        channel: String,
        data: String,
    },
}
