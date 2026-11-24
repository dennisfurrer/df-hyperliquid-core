//! The aligned quote actions' posted JSON and the rate a vote refuses. Hashes and signatures
//! against `@nktkas/hyperliquid` are in `tests/conformance/deploy.rs`.

use super::*;

#[test]
fn a_rate_vote_is_sent_in_normal_form_and_never_negative() {
    let vote = ValidatorL1Stream::new(Dec::parse("0.0400").unwrap()).unwrap();
    assert_eq!(vote.wire(), r#"{"type":"validatorL1Stream","riskFreeRate":"0.04"}"#);
    assert!(ValidatorL1Stream::new(Dec::ZERO).is_ok());
    assert_eq!(
        ValidatorL1Stream::new(Dec::parse("-0.01").unwrap()),
        Err(BadDeploy::OutOfRange { what: "a risk-free rate", least: "0", most: "any", value: "-0.01".to_owned() })
    );
}

#[test]
fn a_role_is_technical_or_treasury() {
    let role = |role| AuthorizeAqav2Role { token: 0, role }.wire();
    assert_eq!(role(AqaRole::Technical), r#"{"type":"authorizeAqav2Role","token":0,"role":"technical"}"#);
    assert_eq!(role(AqaRole::Treasury), r#"{"type":"authorizeAqav2Role","token":0,"role":"treasury"}"#);
}
