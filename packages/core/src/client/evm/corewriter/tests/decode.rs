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
