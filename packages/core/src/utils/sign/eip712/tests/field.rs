//! The wider numbers HyperEVM contracts sign.

use super::*;
use crate::utils::sign::eip712::{Domain, TypedStruct, digest_of_wallet_json};

/// The wider numbers HyperEVM contracts sign: their words, how a wallet is
/// handed them (a `uint128` or `uint256` as a decimal string, as viem sends a
/// bigint), and a `uint256[]` as the keccak of its words. Each was checked
/// against viem.
#[test]
fn the_wide_numbers_encode_as_solidity_does() {
    assert_eq!(Field::Uint32(7).encode(), uint256(7));
    assert_eq!(Field::Uint128(u128::MAX).encode(), uint256(u128::MAX));
    let ids = [uint256(1), uint256(2)];
    let array = Field::Uint256Array(&ids);
    assert_eq!(array.solidity_type(), "uint256[]");
    assert_eq!(array.encode(), keccak256(&[uint256(1), uint256(2)].concat()));
    assert_eq!(Field::Uint256Array(&[]).encode(), keccak256(&[]));
    assert_eq!(Field::Uint128(10).to_json(), Json::String("10".to_owned()));
    assert_eq!(Field::Uint32(10).to_json(), Json::Number("10".to_owned()));
    assert_eq!(array.to_json().compact(), r#"["1","2"]"#);

    let data = TypedStruct::new("Close")
        .field("user", Field::Address(Address::ZERO))
        .field("positionIds", array)
        .field("size", Field::Uint128(u128::MAX))
        .field("leverage", Field::Uint32(1000))
        .field("deadline", Field::Uint256(uint256(1_791_864_000)));
    let domain = Domain::EXCHANGE;
    assert_eq!(digest_of_wallet_json(&data.wallet_json(&domain)), Ok(data.digest(&domain)));
    let json = data.wallet_json(&domain);
    let as_number = json.replacen(r#""1791864000""#, "1791864000", 1);
    assert_eq!(digest_of_wallet_json(&as_number), Ok(data.digest(&domain)), "a number either way");
    let too_wide = json.replacen(r#""leverage":1000"#, r#""leverage":4294967296"#, 1);
    assert!(digest_of_wallet_json(&too_wide).is_err(), "past a uint32");
}
