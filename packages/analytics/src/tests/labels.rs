//! hip4-tui's `tui/src/tests/builders.rs`, the tests of its curated list.

use super::*;

const OUTCOME: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn a_curated_builder_is_named_and_an_uncurated_one_is_not() {
    assert_eq!(label(address(OUTCOME)), Some("OUTCOME"));
    assert_eq!(label(address(&OUTCOME.to_uppercase())), Some("OUTCOME"));
    assert_eq!(label(address("0x1c64d7e3386e005e03ea7a5e669eee3f63183a47")), None, "listed, with no name");
    assert_eq!(kind(address("0x1c64d7e3386e005e03ea7a5e669eee3f63183a47")), Some(Kind::Personal));
    assert_eq!(label(Address::ZERO), None);
    assert_eq!(kind(Address::ZERO), None);
    assert_eq!(kind(address("0x6967d7acd1ec5f210e9590666498e0cc5d13a843")), Some(Kind::Premium));
    assert_eq!(label(address("0xb84168cf3be63c6b8dad05ff5d755e97432ff80b")), Some("PHANTOM"));
    assert_eq!(kind(address("0xb84168cf3be63c6b8dad05ff5d755e97432ff80b")), Some(Kind::Perp));
}

#[test]
fn a_builder_nobody_has_classified_claims_no_classification() {
    assert_eq!(kind(address("0x19d5bd0d8e29bfda20e6d7886a025a89b1758fe1")), None);
    assert_eq!(Kind::Unknown.label(), "");
    assert_eq!(Kind::Real.label(), "product");
    assert_eq!(Kind::Perp.label(), "perp venue");
}

#[test]
fn the_list_is_the_fifty_nine_addresses_it_was_ported_with_each_once() {
    assert_eq!(LABELS.len(), 59);
    assert_eq!(known().count(), 59, "every one reads as an address");
    for one in LABELS {
        assert_eq!(one.address, one.address.to_ascii_lowercase(), "kept lowercase");
        assert_eq!(address(one.address).to_hex(), one.address);
    }
    let mut seen: Vec<Address> = known().collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 59, "no address twice");
    assert_eq!(LABELS.iter().filter(|one| !one.name.is_empty()).count(), 15, "named");
    assert!(SOURCE.contains("builder-meta.ts") && AS_OF == "2026-09-18");
}
