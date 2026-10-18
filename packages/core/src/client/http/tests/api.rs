//! What the meter is handed for each post: a read's kind and, for a read charged per item, the
//! items its answer lists; an action's batch. The gate itself is held through whole journeys in
//! `tests/mocked/client.rs`.

use crate::client::http::limit::LIMIT;
use crate::client::http::transport::{EXCHANGE, INFO, Transport};
use crate::client::{Client, Network};
use crate::test_support::{Wire, block_on};

fn clock() -> u64 {
    0
}

#[test]
fn every_post_goes_to_the_client_s_network_and_is_weighed_by_what_it_carries() {
    let fills = format!("[{}]", vec!["{}"; 45].join(","));
    let orders = format!(r#"{{"action":{{"type":"order","orders":[{}]}},"nonce":1}}"#, vec!["{}"; 45].join(","));
    for (network, api) in
        [(Network::Mainnet, "https://api.hyperliquid.xyz"), (Network::Testnet, "https://api.hyperliquid-testnet.xyz")]
    {
        let client = Client::reader(Wire::always(&fills), network, clock);
        block_on(client.post(INFO, r#"{"type":"l2Book"}"#.to_owned())).unwrap();
        block_on(client.post(INFO, r#"{"type":"userFills"}"#.to_owned())).unwrap();
        block_on(client.post(EXCHANGE, orders.clone())).unwrap();
        let sent = client.http().bodies();
        let urls: Vec<&str> = sent.iter().map(|(url, _)| url.as_str()).collect();
        assert_eq!(urls, [format!("{api}/info"), format!("{api}/info"), format!("{api}/exchange")]);
        assert_eq!(sent[2].1, orders, "{network}");
        let spent = client.spent();
        assert_eq!((spent.limit, spent.minute), (LIMIT, 2 + 22 + 2), "{network}");
        let kinds: Vec<(&str, u32)> = spent.kinds.iter().map(|kind| (kind.name.as_str(), kind.weight)).collect();
        assert_eq!(kinds, [("userFills", 22), ("exchange", 2), ("l2Book", 2)], "only what is charged per item");
    }
}
