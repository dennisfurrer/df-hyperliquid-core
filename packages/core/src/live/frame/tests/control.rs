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
    assert_eq!(
        (subscribed.post_id(), subscribed.subscription()),
        (None, Some(Subscription::Trades { coin: "BTC".to_owned() }))
    );
    assert_eq!(
        refusal("error.alreadyUnsubscribed").subscription(),
        Some(Subscription::Trades { coin: "SOL".to_owned() })
    );
    assert_eq!(
        refusal("error.unsubscribe.fast").subscription(),
        Some(Subscription::L2Book { coin: "SOL".to_owned(), n_sig_figs: None, mantissa: None, fast: false })
    );
    assert_eq!(refusal("error.interval").subscription(), None, "7m is no interval");
    let post = refusal("error.post");
    assert_eq!((post.post_id(), post.subscription()), (Some(2), None));
    for name in ["error.notJson", "error.webData2", "error.invalidSubscription"] {
        assert_eq!((refusal(name).post_id(), refusal(name).subscription()), (None, None), "{name}");
    }
    // @nktkas/hyperliquid 0.33.2 reads a post's id off the end of a refusal such as this one.
    let Frame::Error(pending) = read(r#"{"channel":"error","data":"too many pending post requests id=1234"}"#) else {
        panic!("a refusal")
    };
    assert_eq!(pending.post_id(), Some(1234));
}
