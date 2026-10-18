//! A subscription as the object a subscribe frame carries: written as the venue took it, and read
//! back from it whatever the key order.

use crate::live::Subscription;
use crate::live::subscription::fixtures::sent;
use crate::markets::Interval;
use crate::utils::Json;

#[test]
fn every_subscription_is_written_as_it_was_sent_and_reads_back_from_its_own_object() {
    for (subscription, text) in sent() {
        assert_eq!(subscription.to_json().compact(), text);
        assert_eq!(subscription.to_string(), text);
        assert_eq!(subscription.check(), Ok(()), "{text}");
        assert_eq!(Subscription::parse(&Json::parse(&text).unwrap()), Ok(subscription), "{text}");
    }
    let shuffled = Json::parse(r#"{"interval":"1M","coin":"ETH","type":"candle"}"#).unwrap();
    assert_eq!(
        Subscription::parse(&shuffled),
        Ok(Subscription::Candle { coin: "ETH".to_owned(), interval: Interval::OneMonth })
    );
}
