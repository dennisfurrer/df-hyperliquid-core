use super::*;
use crate::live::fixtures::{all, read};

#[test]
fn every_recorded_frame_reads_and_none_is_left_unknown() {
    for (name, text) in all() {
        let frame = decode(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!matches!(frame, Frame::Other { .. }), "{name} fell through to Other");
    }
}

#[test]
fn an_unknown_channel_is_kept_as_written_and_is_not_an_error() {
    assert_eq!(
        read(r#"{"channel":"somethingNew","data":{"x":[1,2.50,"3"]}}"#),
        Frame::Other { channel: "somethingNew".to_owned(), data: r#"{"x":[1,2.50,"3"]}"#.to_owned() }
    );
    assert_eq!(read(r#"{"channel":"quiet"}"#), Frame::Other { channel: "quiet".to_owned(), data: String::new() });
}

#[test]
fn what_does_not_read_is_an_error_naming_the_channel() {
    for text in ["", "not json", "Websocket connection established.", "[]", r#"{"data":{}}"#, r#"{"channel":7}"#] {
        let error = decode(text).unwrap_err();
        assert_eq!(error.channel, None, "{text:?}");
    }
    for (text, channel) in [
        (r#"{"channel":"l2Book","data":{"coin":"BTC"}}"#, "l2Book"),
        (r#"{"channel":"l2Book"}"#, "l2Book"),
        (
            r#"{"channel":"trades","data":[{"coin":"BTC","side":"X","px":"1","sz":"1","time":1,"hash":"0x","tid":1,"users":["0x42f1b36fb856769e075dfbf25938464fb3c92c70","0x78dce96236804d6958bf3c4598cf5efbc371629c"]}]}"#,
            "trades",
        ),
        // A price written as a float has been through one; it is refused, never rounded.
        (r#"{"channel":"bbo","data":{"coin":"BTC","time":1,"bbo":[{"px":80459.5,"sz":"1","n":1},null]}}"#, "bbo"),
        (
            r#"{"channel":"candle","data":{"t":1,"T":2,"s":"BTC","i":"7m","o":"1","c":"1","h":"1","l":"1","v":"1","n":1}}"#,
            "candle",
        ),
        (r#"{"channel":"post","data":{"id":1,"response":{"type":"other","payload":{}}}}"#, "post"),
        (
            r#"{"channel":"subscriptionResponse","data":{"method":"resubscribe","subscription":{}}}"#,
            "subscriptionResponse",
        ),
    ] {
        let error = decode(text).unwrap_err();
        assert_eq!(error.channel.as_deref(), Some(channel), "{text}");
    }
}

#[test]
fn no_cut_of_a_recorded_frame_panics() {
    for (name, text) in all() {
        let step = if text.len() > 2_000 { 97 } else { 1 };
        for end in (0..text.len()).step_by(step).filter(|end| text.is_char_boundary(*end)) {
            let _ = decode(&text[..end]);
        }
        let _ = decode(&text.replace('"', "'"));
        assert!(decode(&text).is_ok(), "{name}");
    }
}
