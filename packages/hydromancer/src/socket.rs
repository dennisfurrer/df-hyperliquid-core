//! Hydromancer's socket without the socket: the frames to send and what each frame received
//! says. One socket carries every channel; this reads the two used here, `l4BookUpdates` (one
//! frame a block, with every change to the subscribed books) and `builderFills`.
//!
//! - **The key goes in the URL** ([`Hydromancer::socket_url`](crate::Hydromancer::socket_url)):
//!   a browser's socket takes no header, so that is how the provider authenticates one.
//! - **The provider pings every sixty seconds** and closes a socket that does not answer
//!   [`PONG`].
//! - **An L4 frame is read whole or not at all.** A frame with one change that does not read is
//!   refused entire: its block is then missing, and the height gap that leaves makes the
//!   [`Assembler`](crate::Assembler) start again from a snapshot, which is the only honest repair.
//!   Applying the rest of the block would leave a book that looks right and is not.

use hl_core::{Address, Dec, Json};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::snapshot::{Order, Side};

/// The heartbeat a client may send to keep a quiet socket open.
pub const PING: &str = r#"{"type":"ping"}"#;

/// The answer to the provider's ping.
pub const PONG: &str = r#"{"type":"pong"}"#;

fn text(said: &str) -> Json {
    Json::String(said.to_owned())
}

fn list(coins: &[&str]) -> Json {
    Json::Array(coins.iter().map(|coin| text(coin)).collect())
}

/// Subscribes to the L4 changes of `coins` (at most 200). `merge` is pinned to `false`, as the
/// provider asks of every coin-list subscription ahead of changing its default: each subscribe
/// is then its own stream, one frame a block, however the default moves.
#[must_use]
pub fn subscribe_l4(coins: &[&str]) -> String {
    Json::Object(vec![
        ("method".to_owned(), text("subscribe")),
        (
            "subscription".to_owned(),
            Json::Object(vec![
                ("type".to_owned(), text("l4BookUpdates")),
                ("coins".to_owned(), list(coins)),
                ("merge".to_owned(), Json::Bool(false)),
            ]),
        ),
    ])
    .compact()
}

/// Drops `coins` from the L4 stream. `merge` is refused on an unsubscribe.
#[must_use]
pub fn unsubscribe_l4(coins: &[&str]) -> String {
    Json::Object(vec![
        ("method".to_owned(), text("unsubscribe")),
        (
            "subscription".to_owned(),
            Json::Object(vec![("type".to_owned(), text("l4BookUpdates")), ("coins".to_owned(), list(coins))]),
        ),
    ])
    .compact()
}

fn fills_frame(operation: &str, builder: Address) -> String {
    Json::Object(vec![
        ("type".to_owned(), text(operation)),
        (
            "subscription".to_owned(),
            Json::Object(vec![
                ("type".to_owned(), text("builderFills")),
                ("builder".to_owned(), text(&builder.to_hex())),
            ]),
        ),
    ])
    .compact()
}

/// Subscribes to every fill that names `builder`.
#[must_use]
pub fn subscribe_fills(builder: Address) -> String {
    fills_frame("subscribe", builder)
}

#[must_use]
pub fn unsubscribe_fills(builder: Address) -> String {
    fills_frame("unsubscribe", builder)
}

/// What one frame says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// `connected` or `reconnected`: the socket is open for the key.
    Welcome,
    /// The provider's heartbeat: send [`PONG`].
    Ping,
    /// A subscription was taken or dropped.
    Ack,
    /// The provider refused something, in its own words. Pass them through
    /// [`Hydromancer::scrub`](crate::Hydromancer::scrub) before showing them anywhere.
    Refused(String),
    /// One block's changes to the subscribed L4 books.
    Block(Block),
    /// Fills that named a subscribed builder.
    Fills(Vec<BuilderFill>),
    /// A frame of another kind, named; nothing to act on here.
    Other(String),
}

/// One block's changes. Every subscription gets a frame for every block, with no diffs when none
/// of its coins changed, so heights run on without a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub height: u64,
    /// The block's time, Unix milliseconds.
    pub time: u64,
    /// The coin set of the subscription this frame belongs to; `None` on the all-markets stream.
    pub coins: Option<Vec<String>>,
    pub diffs: Vec<Diff>,
}

/// One change to one order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diff {
    /// An order arrived. A maker that paid for priority (ALO) joins the queue in front of
    /// `insert_before` rather than at its back.
    New { coin: String, order: Order, insert_before: Option<u64> },
    /// A resting order's size is now `sz`: a partial fill.
    Update { coin: String, oid: u64, sz: Dec },
    /// Filled or cancelled.
    Remove { coin: String, oid: u64 },
}

impl Diff {
    #[must_use]
    pub fn coin(&self) -> &str {
        match self {
            Diff::New { coin, .. } | Diff::Update { coin, .. } | Diff::Remove { coin, .. } => coin,
        }
    }
}

/// One fill that named the builder, and the trader behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderFill {
    pub user: Address,
    pub coin: String,
    pub px: Dec,
    pub sz: Dec,
    pub side: Side,
    /// Unix milliseconds.
    pub time: u64,
    /// `Buy`, `Sell`, `Open Long` and the like.
    pub dir: Option<String>,
    /// What the fill paid the builder; `None` where it paid nothing.
    pub builder_fee: Option<Dec>,
    pub builder: Option<Address>,
    /// With `time`, what a fill replayed after a reconnect is told apart by.
    pub tx_index: u32,
    pub oid: Option<u64>,
    pub tid: Option<u64>,
    pub hash: Option<String>,
    pub fee: Option<Dec>,
    pub fee_token: Option<String>,
    /// The HIP-4 venue of an outcome market; absent for validator-deployed outcomes.
    pub venue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unreadable frame: {0}")]
pub struct BadFrame(pub String);

/// Every frame's envelope. Live data carries both `type` and `channel`, with the same value. The
/// rest is kept raw and read only for the kinds it belongs to, so a field of another shape on a
/// frame of another kind never fails it.
#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(borrow, default)]
    message: Option<&'a RawValue>,
    #[serde(borrow, default)]
    coins: Option<&'a RawValue>,
    #[serde(borrow, default)]
    data: Option<&'a RawValue>,
    #[serde(borrow, default)]
    fills: Option<&'a RawValue>,
}

/// Reads one text frame. A frame of a kind not read here is [`Message::Other`]; a frame that is
/// not JSON, or an L4 or fills frame that does not read in full, is refused.
pub fn read(text: &str) -> Result<Message, BadFrame> {
    let bad = |error: serde_json::Error| BadFrame(error.to_string());
    let envelope: Envelope<'_> = serde_json::from_str(text).map_err(bad)?;
    let kind = envelope.kind.or(envelope.channel).unwrap_or_default();
    match kind.as_str() {
        "connected" | "reconnected" => Ok(Message::Welcome),
        "ping" => Ok(Message::Ping),
        "subscriptionUpdate" | "subscriptionResponse" => Ok(Message::Ack),
        "error" => {
            let said = envelope
                .message
                .map(|raw| serde_json::from_str::<String>(raw.get()).unwrap_or_else(|_| raw.get().to_owned()));
            Ok(Message::Refused(said.unwrap_or_default()))
        }
        "l4BookUpdates" => {
            let data = envelope.data.ok_or_else(|| BadFrame("an l4BookUpdates frame with no data".to_owned()))?;
            let raw: RawBlock = serde_json::from_str(data.get()).map_err(bad)?;
            let coins = envelope.coins.map(|raw| serde_json::from_str(raw.get())).transpose().map_err(bad)?;
            let diffs = raw.diffs.into_iter().map(RawDiff::read).collect::<Result<_, _>>()?;
            Ok(Message::Block(Block { height: raw.height, time: raw.timestamp, coins, diffs }))
        }
        "builderFills" => {
            let Some(fills) = envelope.fills else { return Ok(Message::Fills(Vec::new())) };
            let raw: Vec<(String, RawFill)> = serde_json::from_str(fills.get()).map_err(bad)?;
            raw.into_iter().map(|(user, fill)| fill.read(&user)).collect::<Result<_, _>>().map(Message::Fills)
        }
        _ => Ok(Message::Other(kind)),
    }
}

#[derive(Deserialize)]
struct RawBlock {
    height: u64,
    timestamp: u64,
    diffs: Vec<RawDiff>,
}

/// A diff as sent. An unknown `type` fails here, and with it the frame.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum RawDiff {
    New {
        coin: String,
        oid: u64,
        user: String,
        side: String,
        px: String,
        sz: String,
        #[serde(default, rename = "insertBefore")]
        insert_before: Option<u64>,
    },
    Update {
        coin: String,
        oid: u64,
        sz: String,
    },
    Remove {
        coin: String,
        oid: u64,
    },
}

fn decimal(said: &str, what: &str) -> Result<Dec, BadFrame> {
    Dec::parse(said).map_err(|_| BadFrame(format!("{what} `{said}` is no decimal")))
}

fn address(said: &str, what: &str) -> Result<Address, BadFrame> {
    said.parse().map_err(|_| BadFrame(format!("{what} `{said}` is no address")))
}

fn side(said: &str) -> Result<Side, BadFrame> {
    Side::parse(said).ok_or_else(|| BadFrame(format!("side `{said}` is neither B nor A")))
}

impl RawDiff {
    fn read(self) -> Result<Diff, BadFrame> {
        Ok(match self {
            RawDiff::New { coin, oid, user, side: said, px, sz, insert_before } => Diff::New {
                order: Order {
                    oid,
                    user: address(&user, "user")?,
                    side: side(&said)?,
                    px: decimal(&px, "price")?,
                    sz: decimal(&sz, "size")?,
                },
                coin,
                insert_before,
            },
            RawDiff::Update { coin, oid, sz } => Diff::Update { coin, oid, sz: decimal(&sz, "size")? },
            RawDiff::Remove { coin, oid } => Diff::Remove { coin, oid },
        })
    }
}

/// A fill as sent: decimals as strings, which [`Dec`] reads as they are. A float would fail.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFill {
    coin: String,
    px: Dec,
    sz: Dec,
    side: String,
    time: u64,
    #[serde(default)]
    dir: Option<String>,
    #[serde(default)]
    builder_fee: Option<Dec>,
    #[serde(default)]
    builder: Option<String>,
    #[serde(default)]
    tx_index: u32,
    #[serde(default)]
    oid: Option<u64>,
    #[serde(default)]
    tid: Option<u64>,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    fee: Option<Dec>,
    #[serde(default)]
    fee_token: Option<String>,
    #[serde(default)]
    venue: Option<String>,
}

impl RawFill {
    fn read(self, user: &str) -> Result<BuilderFill, BadFrame> {
        Ok(BuilderFill {
            user: address(user, "trader")?,
            side: side(&self.side)?,
            builder: self.builder.as_deref().map(|said| address(said, "builder")).transpose()?,
            coin: self.coin,
            px: self.px,
            sz: self.sz,
            time: self.time,
            dir: self.dir,
            builder_fee: self.builder_fee,
            tx_index: self.tx_index,
            oid: self.oid,
            tid: self.tid,
            hash: self.hash,
            fee: self.fee,
            fee_token: self.fee_token,
            venue: self.venue,
        })
    }
}

#[cfg(test)]
#[path = "tests/socket.rs"]
mod tests;
