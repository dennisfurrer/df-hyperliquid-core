//! Recorded payloads read back and rebuilt (`evm_cases.json`), and what the docs' table does
//! not have.

use crate::client::evm::corewriter::support::{action, hex};
use crate::client::evm::fixtures;

use super::*;

#[test]
fn recorded_payloads_and_calldata_are_rebuilt() {
    let file = fixtures::cases();
    for case in file["corewriter"]["encoded"].as_array().unwrap() {
        let payload = fixtures::bytes(&case["payload"]);
        let read = Action::decode(&payload).unwrap();
        assert_eq!(u64::from(read.id()), case["id"].as_u64().unwrap());
        assert_eq!(read.payload(), payload);
        if let Some(values) = case.get("values") {
            assert_eq!(action(case["id"].as_u64().unwrap(), values.as_array().unwrap()), read);
        }
        if let Some(calldata) = case.get("calldata") {
            assert_eq!(hex(&read.calldata()), fixtures::text(calldata));
        }
    }
}

#[test]
fn what_the_table_does_not_have_is_called_out() {
    assert_eq!(Action::decode(&[1, 0, 0, 14]), Err(BadAction::Gap));
    assert_eq!(Action::decode(&[1, 0, 0, 99]), Err(BadAction::UnknownId(99)));
    assert_eq!(Action::decode(&[1, 0]), Err(BadAction::Short(2)));
    assert_eq!(Action::decode(&[2, 0, 0, 4]), Err(BadAction::Version(2)));
    let deposit = Action::StakingDeposit { wei: 5 }.payload();
    assert!(matches!(Action::decode(&deposit[..20]), Err(BadAction::Abi(_))));
    // An enum value the docs do not name still decodes, and encodes back the same.
    let odd = Action::LimitOrder {
        asset: 0,
        is_buy: true,
        limit_px: 1,
        sz: 1,
        reduce_only: false,
        tif: Tif::Other(9),
        cloid: 0,
    };
    assert_eq!(Action::decode(&odd.payload()), Ok(odd));
}

#[test]
fn fields_that_are_not_the_tables_are_refused() {
    let deposit = Action::StakingDeposit { wei: 5 }.tokens();
    assert_eq!(Action::from_tokens(4, &deposit), Ok(Action::StakingDeposit { wei: 5 }));
    assert_eq!(Action::from_tokens(14, &deposit), Err(BadAction::Gap));
    assert_eq!(Action::from_tokens(99, &deposit), Err(BadAction::UnknownId(99)));
    assert_eq!(Action::from_tokens(4, &[]), Err(BadAction::Abi(AbiError::Count { wants: 1, got: 0 })));
    assert!(matches!(Action::from_tokens(4, &[Token::Bool(true)]), Err(BadAction::Abi(_))), "a bool for a uint64");
    // 256 does not fit encodedTif's uint8.
    let order =
        Action::LimitOrder { asset: 0, is_buy: true, limit_px: 1, sz: 1, reduce_only: false, tif: Tif::Gtc, cloid: 0 };
    let mut wide = order.tokens();
    wide[5] = Token::uint(256u64);
    assert!(matches!(Action::from_tokens(1, &wide), Err(BadAction::Abi(_))));
    assert_eq!(Action::from_tokens(1, &order.tokens()), Ok(order));
}
