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
fn splitting_and_merging_hash_to_what_the_venue_s_ui_signs() {
    let split = SplitOutcome::new(1210, dec("12.5")).unwrap();
    assert_eq!(split.wire(), r#"{"type":"userOutcome","splitOutcome":{"outcome":1210,"amount":"12.5"}}"#);
    let merge = MergeOutcome::new(1210, dec("12.5")).unwrap();
    assert_eq!(merge.wire(), r#"{"type":"userOutcome","mergeOutcome":{"outcome":1210,"amount":"12.5"}}"#);
    let all = MergeOutcome::all(1210);
    assert_eq!(
        all.wire(),
        r#"{"type":"userOutcome","mergeOutcome":{"outcome":1210,"amount":null}}"#,
        "a nil, not a figure"
    );
    let vault: Address = "0x1234567890abcdef1234567890abcdef12345678".parse().unwrap();
    for (action, nonce, vault, expected) in [
        (&split as &dyn L1Action, NONCE, None, "1445be10b18e77a5957529ec5e646debd54581e0268d12898353fc3a9977167a"),
        (
            &SplitOutcome::new(1210, dec("1000")).unwrap(),
            NONCE,
            None,
            "570385f989587efa2530636b4a576071ffca08550aff252044575ffa2997c1f3",
        ),
        (
            &SplitOutcome::new(7, dec("0.000001")).unwrap(),
            1,
            None,
            "db98760463374704f825e56c9d3f6c9886897e8e8c1291a225aee4d3f34a795d",
        ),
        (&merge, NONCE, None, "1f65c6ac80c7c45a71abb81729d28b7d6f558c546c077a32688b566980022a33"),
        (&all, NONCE, None, "689ae9d8e81b55b07306707f104f857232e4f99ffdaeb04753d483fcc19906db"),
        (&split, NONCE, Some(vault), "742588ad20e35e27d5b9d03d3bf346f2ffa8266642fdea0aa19ff33128f1c415"),
    ] {
        assert_eq!(id(action, nonce, vault), expected, "{}", action.wire());
    }
}

#[test]
fn an_amount_is_positive_with_at_most_eight_places_and_trailing_zeros_are_one_action() {
    let plain = SplitOutcome::new(1210, dec("12.5")).unwrap();
    let padded = SplitOutcome::new(1210, dec("12.50000000")).unwrap();
    assert_eq!(plain.msgpack(), padded.msgpack());
    for bad in ["-1", "0", "0.000000001"] {
        assert_eq!(SplitOutcome::new(1, dec(bad)), Err(OutcomeError::Amount(dec(bad))), "{bad}");
    }
    assert_eq!(MergeOutcome::new(1210, Dec::ZERO), Err(OutcomeError::Amount(Dec::ZERO)), "zero is not everything");
}

/// A question merge and a negation hold their amounts to what a split takes; a merge of every set
/// posts nil.
#[test]
fn a_question_merge_and_a_negation_hold_their_amounts_as_a_split_does() {
    assert_eq!(MergeQuestion::new(51, dec("0")), Err(OutcomeError::Amount(dec("0"))));
    assert_eq!(NegateOutcome::new(51, 1473, dec("0.000000001")), Err(OutcomeError::Amount(dec("0.000000001"))));
    assert_eq!(
        MergeQuestion::all(51).wire(),
        r#"{"type":"userOutcome","mergeQuestion":{"question":51,"amount":null}}"#
    );
    assert_eq!(
        NegateOutcome::new(51, 1473, dec("3.10")).unwrap().wire(),
        r#"{"type":"userOutcome","negateOutcome":{"question":51,"outcome":1473,"amount":"3.1"}}"#
    );
}
