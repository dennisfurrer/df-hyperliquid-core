//! The websocket on mainnet, read only: one subscription of every kind the crate serves, market
//! data and the HLP vault's account, and one info read posted over the socket. The venue must
//! acknowledge each subscription as the one sent, the channel's first frame must read into its own
//! [`Frame`] variant, and its data must have the shape of the frame recorded for it under
//! `fixtures/ws`, so a field the venue has added shows up here first. Where the HLP vault's frames
//! carry what the recorded accounts' leave out (the vaults it leads, deposits into it and
//! withdrawals from it, the snapshot that opens its order history), they are held against its own
//! recording as well.
//!
//! The feed hands back typed frames only, so the subscriptions run through a tap ([`tapped`]): a
//! local socket between the feed and the venue that keeps each text frame the venue sends.

mod tapped;

use std::path::PathBuf;

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::native::Feed;
#[cfg(feature = "__test-internals")]
use df_hyperliquid_core::live::reads_directly;
use df_hyperliquid_core::live::{Answer, Event, Frame, Post, Session, Subscription};
use df_hyperliquid_core::markets::{Book, Interval};
use df_hyperliquid_core::utils::Address;
use serde_json::Value;
use tokio::time::{Instant, timeout_at};

use self::tapped::{CHANNEL, Tapped};
use crate::shape::added;
use crate::support::fixture;

/// The HLP vault.
const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";

/// Whether a frame is the variant its channel reads into.
type Reads = fn(&Frame) -> bool;

/// A subscription, the recordings its data is held against (`<manifest>#<frame>`, several joined
/// by `+`, a path new to all of them being added; `user` for `userEvents`, whose recording depends
/// on the event), what its frame reads into, and whether it sends at once or only when something
/// happens.
type Check = (Subscription, &'static str, Reads, Sends);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sends {
    AtOnce,
    /// Only when something happens (a notification, an order changing, an outcome settling):
    /// nothing within the wait is no failure.
    OnChange,
}

#[tokio::test]
#[ignore = "mainnet"]
async fn every_market_channel_reads_and_keeps_its_recorded_shape() {
    let coin = |name: &str| name.to_owned();
    let checks: [Check; 15] = [
        (
            Subscription::AllMids { dex: String::new() },
            "mainnet#allMids",
            |f| matches!(f, Frame::AllMids(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::L2Book { coin: coin("BTC"), n_sig_figs: None, mantissa: None, fast: false },
            "mainnet#l2Book",
            |f| matches!(f, Frame::Book(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::Trades { coin: coin("BTC") },
            "mainnet#trades",
            |f| matches!(f, Frame::Trades(_)),
            Sends::AtOnce,
        ),
        (Subscription::Bbo { coin: coin("BTC") }, "mainnet#bbo", |f| matches!(f, Frame::Bbo(_)), Sends::AtOnce),
        (
            Subscription::Candle { coin: coin("BTC"), interval: Interval::OneMinute },
            "mainnet#candle",
            |f| matches!(f, Frame::Candle(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::ActiveAssetCtx { coin: coin("BTC") },
            "mainnet#activeAssetCtx",
            |f| matches!(f, Frame::AssetCtx { .. }),
            Sends::AtOnce,
        ),
        (
            Subscription::ActiveAssetCtx { coin: coin("@107") },
            "mainnet#activeSpotAssetCtx",
            |f| matches!(f, Frame::SpotAssetCtx { .. }),
            Sends::AtOnce,
        ),
        (
            Subscription::AllDexsAssetCtxs,
            "mainnet#allDexsAssetCtxs",
            |f| matches!(f, Frame::AllDexsAssetCtxs(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::AssetCtxs { dex: coin("xyz") },
            "mainnet#assetCtxs",
            |f| matches!(f, Frame::AssetCtxs { .. }),
            Sends::AtOnce,
        ),
        (Subscription::SpotAssetCtxs, "mainnet#spotAssetCtxs", |f| matches!(f, Frame::SpotAssetCtxs(_)), Sends::AtOnce),
        (Subscription::FastAssetCtxs, "mainnet#fastAssetCtxs", |f| matches!(f, Frame::FastAssetCtxs(_)), Sends::AtOnce),
        (
            Subscription::L2 { coin: coin("BTC"), n_sig_figs: None, mantissa: None },
            "compressed#l2",
            |f| matches!(f, Frame::L2(_)),
            Sends::AtOnce,
        ),
        (Subscription::Pac, "compressed#pac", |f| matches!(f, Frame::Pac(_)), Sends::AtOnce),
        (Subscription::Sac, "compressed#sac", |f| matches!(f, Frame::Sac(_)), Sends::AtOnce),
        (
            Subscription::OutcomeMetaUpdates,
            "testnet#outcomeMetaUpdates+mainnet#outcomeMetaUpdates.created",
            |f| matches!(f, Frame::OutcomeMetaUpdates(_)),
            Sends::OnChange,
        ),
    ];
    held(checks).await;
}

#[tokio::test]
#[ignore = "mainnet"]
async fn every_account_channel_of_the_hlp_vault_reads_and_keeps_its_recorded_shape() {
    let user: Address = HLP.parse().unwrap();
    let main = String::new;
    let checks: [Check; 16] = [
        (
            Subscription::Notification { user },
            "mainnet#notification",
            |f| matches!(f, Frame::Notification(_)),
            Sends::OnChange,
        ),
        (Subscription::WebData3 { user }, "vault#webData3", |f| matches!(f, Frame::WebData3(_)), Sends::AtOnce),
        (
            Subscription::TwapStates { user, dex: main() },
            "mainnet#twapStates",
            |f| matches!(f, Frame::TwapStates(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::ClearinghouseState { user, dex: main() },
            "mainnet#clearinghouseState",
            |f| matches!(f, Frame::ClearinghouseState(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::OpenOrders { user, dex: main() },
            "mainnet#openOrders",
            |f| matches!(f, Frame::OpenOrders(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::OrderUpdates { user },
            "mainnet#orderUpdates",
            |f| matches!(f, Frame::OrderUpdates(_)),
            Sends::OnChange,
        ),
        (Subscription::UserEvents { user }, "user", |f| matches!(f, Frame::UserEvent(_)), Sends::OnChange),
        (
            Subscription::UserFills { user, aggregate_by_time: false },
            "mainnet#userFills.snapshot",
            |f| matches!(f, Frame::UserFills(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::UserFundings { user },
            "mainnet#userFundings",
            |f| matches!(f, Frame::UserFundings(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::UserNonFundingLedgerUpdates { user },
            "mainnet#userNonFundingLedgerUpdates+vault#userNonFundingLedgerUpdates",
            |f| matches!(f, Frame::Ledger(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::ActiveAssetData { user, coin: "BTC".to_owned() },
            "mainnet#activeAssetData",
            |f| matches!(f, Frame::AssetData(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::UserTwapSliceFills { user },
            "mainnet#userTwapSliceFills+mainnet#userTwapSliceFills.empty",
            |f| matches!(f, Frame::TwapSliceFills(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::UserTwapHistory { user },
            "mainnet#userTwapHistory",
            |f| matches!(f, Frame::TwapHistory(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::SpotState { user, ignore_portfolio_margin: false },
            "mainnet#spotState",
            |f| matches!(f, Frame::SpotState(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::AllDexsClearinghouseState { user },
            "mainnet#allDexsClearinghouseState",
            |f| matches!(f, Frame::AllDexsClearinghouseState(_)),
            Sends::AtOnce,
        ),
        (
            Subscription::UserHistoricalOrders { user },
            "mainnet#userHistoricalOrders+vault#userHistoricalOrders",
            |f| matches!(f, Frame::HistoricalOrders(_)),
            Sends::AtOnce,
        ),
    ];
    held(checks).await;
}

/// The recorded read, `l2Book` of ETH at five figures, posted over the socket as a user would,
/// on a feed dialling mainnet itself. Its answer carries the post's id, reads as a grouped book,
/// and has the shape of the recorded answer.
#[tokio::test]
#[ignore = "mainnet"]
async fn an_info_post_over_the_socket_is_answered_by_its_id_in_the_recorded_shape() {
    let mut feed = Feed::start(Session::new(Network::Mainnet));
    let deadline = Instant::now() + CHANNEL;
    while timeout_at(deadline, feed.next()).await.expect("the socket opens").expect("a running feed") != Event::Opened {
    }
    let read = Post::info(r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}"#).unwrap();
    let id = feed.post(read).await.unwrap();
    let reply = loop {
        match timeout_at(deadline, feed.next()).await.expect("an answer in time").expect("a running feed") {
            Event::Frame(frame) => match *frame {
                Frame::Post(reply) => break reply,
                Frame::Error(refusal) => panic!("the venue refused the post: {}", refusal.message),
                _ => {}
            },
            Event::Closed { reason, .. } => panic!("the socket closed: {reason}"),
            _ => {}
        }
    };
    assert_eq!(reply.id, id);
    let Answer::Info { kind, data } = reply.answer else { panic!("an info answer: {:?}", reply.answer) };
    assert_eq!(kind, "l2Book");
    let book: Book = serde_json::from_str(&data).unwrap();
    let (bid, ask) = book.touch();
    assert_eq!(book.coin, "ETH");
    assert!(book.spread.is_some() && bid.unwrap() < ask.unwrap());
    let live: Value = serde_json::from_str(&data).unwrap();
    let recorded = &frame_of("mainnet#post.info.l2Book")["data"]["response"]["payload"]["data"];
    keep("mainnet#post.info.l2Book", &live);
    let added = added(recorded, &live);
    assert!(added.is_empty(), "the venue now answers l2Book over the socket with fields it did not have: {added:#?}");
}

/// Runs each check in turn on one tapped feed, and fails naming every one that did not hold.
async fn held(checks: impl IntoIterator<Item = Check>) {
    let mut tapped = Tapped::open().await;
    let mut failures = Vec::new();
    for (subscription, recording, reads, sends) in checks {
        let found = match tapped.first(&subscription).await {
            Ok(found) => found,
            Err(why) => {
                failures.push(format!("{subscription}: {why}"));
                continue;
            }
        };
        let Some((frame, text)) = found else {
            if sends == Sends::AtOnce {
                failures.push(format!("{subscription}: no frame within {CHANNEL:?}"));
            }
            continue;
        };
        if !reads(&frame) {
            let read: String = format!("{frame:?}").chars().take(200).collect();
            failures.push(format!("{subscription}: read as {read}"));
        }
        // `just live` builds with `__test-internals`: the venue must still write the layout the
        // direct reader takes, or `decode` has quietly moved these channels to the serde path.
        #[cfg(feature = "__test-internals")]
        if ["l2Book", "trades", "bbo"].contains(&subscription.channel()) && !reads_directly(&text) {
            failures.push(format!("{subscription}: no longer in the layout the direct reader takes"));
        }
        let live: Value = serde_json::from_str(&text).unwrap();
        let recording = match recording {
            "user" => format!("mainnet#user.{}", live["data"].as_object().and_then(|data| data.keys().next()).unwrap()),
            other => other.to_owned(),
        };
        let new_to = |one: &str| added(&frame_of(one)["data"], &live["data"]);
        let mut recordings = recording.split('+');
        let primary = recordings.next().unwrap_or_default();
        keep(primary, &live["data"]);
        let mut fresh = new_to(primary);
        for other in recordings {
            let theirs = new_to(other);
            fresh.retain(|path| theirs.contains(path));
        }
        if !fresh.is_empty() {
            failures.push(format!("{subscription}: the venue now sends fields {recording} did not have: {fresh:#?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A recorded frame, `<manifest>#<name>` under `fixtures/ws`, as JSON.
fn frame_of(recording: &str) -> Value {
    let (manifest, name) = recording.split_once('#').unwrap();
    let file: Value = serde_json::from_str(&fixture(&format!("ws/{manifest}.json"))).unwrap();
    let text = file["frames"][name].as_str().unwrap_or_else(|| panic!("ws/{manifest}.json has no frame {name}"));
    serde_json::from_str(text).unwrap()
}

/// Keeps what the venue sent as `target/tmp/live/ws/<manifest>.<name>.json`, to re-record from.
fn keep(recording: &str, live: &Value) {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("live").join("ws");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{}.json", recording.replace('#', ".")));
    std::fs::write(path, serde_json::to_string_pretty(live).unwrap()).unwrap();
}
