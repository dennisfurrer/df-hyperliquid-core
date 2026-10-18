use crate::live::subscription::fixtures::sent;

#[test]
fn each_subscription_is_written_as_it_was_sent_and_taken() {
    for (subscription, text) in sent() {
        assert_eq!(subscription.to_json().compact(), text);
        assert_eq!(subscription.to_string(), text);
        assert_eq!(subscription.check(), Ok(()), "{text}");
    }
}
