//! Reading a text frame: every recorded frame reads into its own type, and what does not read is
//! an error naming the channel, never a panic and never a figure made up.

use super::*;
use crate::live::fixtures::{all, mainnet, portfolio, read};

/// Every frame recorded under `fixtures/ws` reads, none falls through to [`Frame::Other`], and no
/// cut or mangled copy of one panics.
#[test]
fn every_recorded_frame_reads_and_no_cut_of_one_panics() {
    for (name, text) in all() {
        let frame = decode(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!matches!(frame, Frame::Other { .. }), "{name} fell through to Other");
        let step = if text.len() > 2_000 { 97 } else { 1 };
        for end in (0..text.len()).step_by(step).filter(|end| text.is_char_boundary(*end)) {
            let _ = decode(&text[..end]);
        }
        let _ = decode(&text.replace('"', "'"));
    }
}

/// An unknown channel is kept as written. A frame that is not one names no channel; one whose data
/// does not read names its channel. A price written as a float has been through one, and a field
/// every recorded frame carries (a resting order's type and trigger, the mark, an isolated
/// position's raw USD, a spot balance's entry notional) is never read as absent.
#[test]
fn an_unknown_channel_is_kept_and_what_does_not_read_names_its_channel() {
    assert_eq!(
        read(r#"{"channel":"somethingNew","data":{"x":[1,2.50,"3"]}}"#),
        Frame::Other { channel: "somethingNew".to_owned(), data: r#"{"x":[1,2.50,"3"]}"#.to_owned() }
    );
    assert_eq!(read(r#"{"channel":"quiet"}"#), Frame::Other { channel: "quiet".to_owned(), data: String::new() });

    for text in ["", "not json", "Websocket connection established.", "[]", r#"{"data":{}}"#, r#"{"channel":7}"#] {
        let error = decode(text).unwrap_err();
        assert_eq!(error.channel, None, "{text:?}");
        assert!(error.to_string().starts_with("not a frame"), "{error}");
    }
    let open = mainnet("openOrders");
    let mark = mainnet("activeAssetData");
    let isolated = portfolio("allDexsClearinghouseState");
    let spot = mainnet("spotState");
    let refused = [
        (r#"{"channel":"l2Book","data":{"coin":"BTC"}}"#.to_owned(), "l2Book"),
        (r#"{"channel":"l2Book"}"#.to_owned(), "l2Book"),
        (
            r#"{"channel":"trades","data":[{"coin":"BTC","side":"X","px":"1","sz":"1","time":1,"hash":"0x","tid":1,"users":["0x42f1b36fb856769e075dfbf25938464fb3c92c70","0x78dce96236804d6958bf3c4598cf5efbc371629c"]}]}"#.to_owned(),
            "trades",
        ),
        (
            r#"{"channel":"bbo","data":{"coin":"BTC","time":1,"bbo":[{"px":80459.5,"sz":"1","n":1},null]}}"#.to_owned(),
            "bbo",
        ),
        (
            r#"{"channel":"candle","data":{"t":1,"T":2,"s":"BTC","i":"7m","o":"1","c":"1","h":"1","l":"1","v":"1","n":1}}"#.to_owned(),
            "candle",
        ),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"other","payload":{}}}}"#.to_owned(), "post"),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"info","payload":[]}}}"#.to_owned(), "post"),
        (
            r#"{"channel":"subscriptionResponse","data":{"method":"resubscribe","subscription":{}}}"#.to_owned(),
            "subscriptionResponse",
        ),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"action","payload":{"nope":1}}}}"#.to_owned(), "post"),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"error","payload":429}}}"#.to_owned(), "post"),
        (
            r#"{"channel":"subscriptionResponse","data":{"method":"subscribe","subscription":{"a":1,"a":2}}}"#.to_owned(),
            "subscriptionResponse",
        ),
        (r#"{"channel":"outcomeMetaUpdates","data":{"a":1,"a":2}}"#.to_owned(), "outcomeMetaUpdates"),
        (r#"{"channel":"outcomeMetaUpdates"}"#.to_owned(), "outcomeMetaUpdates"),
        (r#"{"channel":"user","data":{"fills":[],"funding":{}}}"#.to_owned(), "user"),
        (r#"{"channel":"user","data":{}}"#.to_owned(), "user"),
        (open.replace(r#""orderType":"Limit","#, ""), "openOrders"),
        (open.replace(r#""triggerCondition":"N/A","#, ""), "openOrders"),
        (open.replace(r#""isPositionTpsl":false,"#, ""), "openOrders"),
        (mark.replace(r#","markPx":"80454.0""#, ""), "activeAssetData"),
        (isolated.replace(r#","rawUsd":"-3828.81778""#, ""), "allDexsClearinghouseState"),
        (spot.replacen(r#","entryNtl":"#, r#","entry":"#, 1), "spotState"),
    ];
    for (text, channel) in refused {
        let error = decode(&text).unwrap_err();
        assert_eq!(error.channel.as_deref(), Some(channel), "{text}");
        assert!(error.to_string().starts_with(&format!("a `{channel}` frame did not read")), "{error}");
    }
}
