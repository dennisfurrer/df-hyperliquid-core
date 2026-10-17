//! Split and merge connection ids, checked against the venue's own "Convert Outcomes" UI; nonce
//! 1700000000000 unless named.

use crate::utils::{Address, hex};

use super::*;

const NONCE: u64 = 1_700_000_000_000;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn id(action: &dyn L1Action, nonce: u64, vault: Option<Address>) -> String {
    hex::encode(&action.connection_id(nonce, vault))
}

#[test]
fn splitting_and_merging_hash_to_what_the_other_implementation_gets() {
    let split = SplitOutcome::new(1210, dec("12.5")).unwrap();
    assert_eq!(split.wire(), r#"{"type":"userOutcome","splitOutcome":{"outcome":1210,"amount":"12.5"}}"#);
    assert_eq!(id(&split, NONCE, None), "1445be10b18e77a5957529ec5e646debd54581e0268d12898353fc3a9977167a");
    let whole = SplitOutcome::new(1210, dec("1000")).unwrap();
    assert_eq!(id(&whole, NONCE, None), "570385f989587efa2530636b4a576071ffca08550aff252044575ffa2997c1f3");
    let smallest = SplitOutcome::new(7, dec("0.000001")).unwrap();
    assert_eq!(id(&smallest, 1, None), "db98760463374704f825e56c9d3f6c9886897e8e8c1291a225aee4d3f34a795d");
    let merge = MergeOutcome::new(1210, dec("12.5")).unwrap();
    assert_eq!(merge.wire(), r#"{"type":"userOutcome","mergeOutcome":{"outcome":1210,"amount":"12.5"}}"#);
    assert_eq!(id(&merge, NONCE, None), "1f65c6ac80c7c45a71abb81729d28b7d6f558c546c077a32688b566980022a33");
}

#[test]
fn merging_everything_is_a_nil_amount_and_not_a_figure() {
    let all = MergeOutcome::all(1210);
    assert_eq!(all.wire(), r#"{"type":"userOutcome","mergeOutcome":{"outcome":1210,"amount":null}}"#);
    assert_eq!(id(&all, NONCE, None), "689ae9d8e81b55b07306707f104f857232e4f99ffdaeb04753d483fcc19906db");
    assert_eq!(MergeOutcome::new(1210, Dec::ZERO), Err(OutcomeError::Amount(Dec::ZERO)), "zero is not everything");
}

#[test]
fn splitting_as_a_subaccount_carries_the_vault_marker() {
    let vault: Address = "0x1234567890abcdef1234567890abcdef12345678".parse().unwrap();
    let split = SplitOutcome::new(1210, dec("12.5")).unwrap();
    assert_eq!(id(&split, NONCE, Some(vault)), "742588ad20e35e27d5b9d03d3bf346f2ffa8266642fdea0aa19ff33128f1c415");
}

#[test]
fn trailing_zeros_are_not_a_different_action() {
    let plain = SplitOutcome::new(1210, dec("12.5")).unwrap();
    let padded = SplitOutcome::new(1210, dec("12.50000000")).unwrap();
    assert_eq!(plain.msgpack(), padded.msgpack());
    for bad in ["-1", "0", "0.000000001"] {
        assert_eq!(SplitOutcome::new(1, dec(bad)), Err(OutcomeError::Amount(dec(bad))), "{bad}");
    }
}
