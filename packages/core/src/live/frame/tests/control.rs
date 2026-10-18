//! The venue's word on what was sent: acknowledgements naming the subscription they took, pongs,
//! and refusals naming the post or the subscription they refuse when they quote one.

use super::*;
use crate::live::Frame;
use crate::live::fixtures::{address, mainnet, portfolio, read};

#[test]
fn acks_pongs_and_refusals_are_frames_of_their_own() {
    let Frame::Ack(ack) = read(&mainnet("ack.unsubscribe")) else { panic!("an ack") };
    assert_eq!(ack.method, Method::Unsubscribe);
    assert_eq!(ack.echo.compact(), r#"{"type":"trades","coin":"@107"}"#);
    let Frame::Ack(ack) = read(&portfolio("ack.allDexsClearinghouseState")) else { panic!("an ack") };
    assert_eq!(ack.method, Method::Subscribe);
    assert_eq!(
        ack.subscription(),
        Ok(Subscription::AllDexsClearinghouseState { user: address("0x8d1548a936241928cef288a1c377919a5d511f7a") })
    );
    assert_eq!(read(&mainnet("pong")), Frame::Pong);

    let refusal = |name: &str| match read(&mainnet(name)) {
        Frame::Error(refusal) => refusal,
        other => panic!("{name} is {other:?}"),
    };
    let subscribed = refusal("error.alreadySubscribed");
    assert_eq!(subscribed.message, r#"Already subscribed: {"type":"trades","coin":"BTC"}"#);
    let book =
        |coin: &str| Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false };
    for (name, post, subscription) in [
        ("error.alreadySubscribed", None, Some(Subscription::Trades { coin: "BTC".to_owned() })),
        ("error.alreadyUnsubscribed", None, Some(Subscription::Trades { coin: "SOL".to_owned() })),
        ("error.unsubscribe.fast", None, Some(book("SOL"))),
        ("error.interval", None, None),
        ("error.post", Some(2), None),
        ("error.notJson", None, None),
        ("error.webData2", None, None),
        ("error.invalidSubscription", None, None),
    ] {
        let refused = refusal(name);
        assert_eq!((refused.post_id(), refused.subscription()), (post, subscription), "{name}");
    }
    // @nktkas/hyperliquid 0.33.2 reads a post's id off the end of a refusal such as this one.
    let Frame::Error(pending) = read(r#"{"channel":"error","data":"too many pending post requests id=1234"}"#) else {
        panic!("a refusal")
    };
    assert_eq!(pending.post_id(), Some(1234));
}
