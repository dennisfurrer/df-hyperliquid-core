//! The validator and signer actions' posted JSON: null for what a profile change leaves alone, and
//! an IPv6 node in its short form. Hashes and signatures against both reference SDKs are in
//! `tests/conformance/validator.rs`.

use super::*;

const SIGNER: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";

#[test]
fn a_profile_change_sends_null_for_what_it_leaves_alone() {
    let unjail = CValidatorAction::ChangeProfile(ProfileChange { unjailed: true, ..ProfileChange::default() });
    assert_eq!(
        unjail.wire(),
        r#"{"type":"CValidatorAction","changeProfile":{"node_ip":null,"name":null,"description":null,"unjailed":true,"disable_delegations":null,"commission_bps":null,"signer":null}}"#
    );
    assert_eq!(CValidatorAction::Unregister.wire(), r#"{"type":"CValidatorAction","unregister":null}"#);
    assert_eq!(CSignerAction::JailSelf.wire(), r#"{"type":"CSignerAction","jailSelf":null}"#);
    assert_eq!(CSignerAction::UnjailSelf.wire(), r#"{"type":"CSignerAction","unjailSelf":null}"#);
}

#[test]
fn a_registration_writes_the_node_ip_short_and_the_signer_in_lower_case() {
    let profile = ValidatorProfile {
        node_ip: "2001:0db8:0000:0000:0000:0000:0000:0001".parse().unwrap(),
        name: "v".to_owned(),
        description: String::new(),
        delegations_disabled: false,
        commission_bps: 500,
        signer: SIGNER.to_uppercase().replacen("0X", "0x", 1).parse().unwrap(),
    };
    assert_eq!(
        CValidatorAction::Register { profile, unjailed: true, initial_wei: 7 }.wire(),
        format!(
            r#"{{"type":"CValidatorAction","register":{{"profile":{{"node_ip":{{"Ip":"2001:db8::1"}},"name":"v","description":"","delegations_disabled":false,"commission_bps":500,"signer":"{SIGNER}"}},"unjailed":true,"initial_wei":7}}}}"#
        )
    );
}
