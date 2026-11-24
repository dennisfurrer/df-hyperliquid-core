//! The signers a multi-sig conversion names: sorted, lowercase, compact, and held to what the
//! venue takes.

use super::*;

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[test]
fn the_signers_are_written_sorted_lowercase_and_compact() {
    let (a, b) =
        (address("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"), address("0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc"));
    let convert = ConvertToMultiSigUser::new(&[a, b, a], 2, 7).unwrap();
    assert_eq!(
        convert.signers(),
        r#"{"authorizedUsers":["0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc","0x70997970c51812dc3a010c7d01b50e0d17dc79c8"],"threshold":2}"#
    );
    assert_eq!(ConvertToMultiSigUser::single(7).signers(), "null");
    assert_eq!(ConvertToMultiSigUser::KEY, Signers::Main);
}

#[test]
fn too_many_signers_none_or_a_threshold_out_of_reach_is_refused() {
    let users: Vec<Address> = (1..=11u8).map(|byte| Address::from_bytes([byte; 20])).collect();
    assert_eq!(ConvertToMultiSigUser::new(&users, 2, 1), Err(BadSigners::Count(11)));
    assert_eq!(ConvertToMultiSigUser::new(&[], 1, 1), Err(BadSigners::Count(0)));
    assert_eq!(ConvertToMultiSigUser::new(&users[..2], 3, 1), Err(BadSigners::Threshold { threshold: 3, signers: 2 }));
    assert_eq!(ConvertToMultiSigUser::new(&users[..2], 0, 1), Err(BadSigners::Threshold { threshold: 0, signers: 2 }));
    assert!(ConvertToMultiSigUser::new(&users[..10], 10, 1).is_ok());
}
