//! The contract's selector and topic against the hashes they claim to be, and the table
//! against the docs.

use crate::client::evm::corewriter::support::hex;

use super::*;

#[test]
fn the_contract_selector_and_topic_are_the_hashes_they_claim_to_be() {
    assert_eq!(CORE_WRITER.to_hex(), "0x3333333333333333333333333333333333333333");
    assert_eq!(
        RAW_ACTION_TOPIC,
        crate::client::evm::abi::event_topic("event RawAction(address indexed user, bytes data)").unwrap()
    );
    assert_eq!(RAW_ACTION_TOPIC.to_hex(), "0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3");
    assert_eq!(SEND_RAW_ACTION, crate::client::evm::abi::selector("sendRawAction(bytes)").unwrap());
    assert_eq!(hex(&SEND_RAW_ACTION), "0x17938e13");
    let ids: Vec<u32> = SPECS.iter().map(|spec| spec.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17], "the docs' table skips 14");
    for spec in &SPECS {
        let types = crate::client::evm::abi::params(spec.types).unwrap();
        assert_eq!(types.len(), spec.fields.len(), "{}", spec.name);
    }
}
