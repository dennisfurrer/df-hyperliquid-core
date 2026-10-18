//! The two domains' separators, as published.

use super::*;
use crate::utils::hex;

#[test]
fn the_domain_separators_are_the_published_constants() {
    assert_eq!(
        hex::encode(&Domain::EXCHANGE.separator()),
        "d79297fcdf2ffcd4ae223d01edaa2ba214ff8f401d7c9300d995d17c82aa4040"
    );
    assert_eq!(
        hex::encode(&Domain::user_signed(Network::Mainnet).separator()),
        "2f54e643b38c9c2b65cb731aaf11c54f01705ee0cdf45aea16e2ac0a5ce7531a"
    );
    assert_eq!(Domain::EXCHANGE.chain_id, 1337);
    assert_eq!(Domain::user_signed(Network::Mainnet).chain_id, 42_161);
}
