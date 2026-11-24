//! Each `spotDeploy` variant's posted JSON, what is left out when unset, and what the constructors
//! refuse. Hashes and signatures against the reference SDKs are in `tests/conformance/deploy.rs`.

use super::*;

const USER: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn user() -> Address {
    USER.parse().unwrap()
}

#[test]
fn unset_optional_keys_are_left_out_and_set_ones_follow_the_docs() {
    let spec = TokenSpec { name: "TEST".to_owned(), sz_decimals: 2, wei_decimals: 8 };
    let bare = SpotDeploy::RegisterToken2 { spec: spec.clone(), max_gas: 500, full_name: None };
    assert_eq!(
        bare.wire(),
        r#"{"type":"spotDeploy","registerToken2":{"spec":{"name":"TEST","szDecimals":2,"weiDecimals":8},"maxGas":500}}"#
    );
    let named = SpotDeploy::RegisterToken2 { spec, max_gas: 500, full_name: Some("Test".to_owned()) };
    assert!(named.wire().ends_with(r#""maxGas":500,"fullName":"Test"}}"#));

    let genesis = |no_hyperliquidity| SpotDeploy::Genesis { token: 7, max_supply: 10u128.pow(20), no_hyperliquidity };
    assert_eq!(
        genesis(false).wire(),
        r#"{"type":"spotDeploy","genesis":{"token":7,"maxSupply":"100000000000000000000"}}"#
    );
    assert!(genesis(true).wire().ends_with(r#""maxSupply":"100000000000000000000","noHyperliquidity":true}}"#));

    let users = |blacklist_users| SpotDeploy::UserGenesis {
        token: 7,
        user_and_wei: vec![(user(), 5)],
        existing_token_and_wei: vec![(1, 6)],
        blacklist_users,
    };
    assert_eq!(
        users(vec![]).wire(),
        format!(
            r#"{{"type":"spotDeploy","userGenesis":{{"token":7,"userAndWei":[["{USER}","5"]],"existingTokenAndWei":[[1,"6"]]}}}}"#
        )
    );
    assert!(users(vec![(user(), true)]).wire().ends_with(&format!(r#""blacklistUsers":[["{USER}",true]]}}}}"#)));

    let seeded = |levels| SpotDeploy::register_hyperliquidity(3, dec("2.50"), dec("4"), 100, levels).unwrap();
    assert_eq!(
        seeded(None).wire(),
        r#"{"type":"spotDeploy","registerHyperliquidity":{"spot":3,"startPx":"2.5","orderSz":"4","nOrders":100}}"#
    );
    assert!(seeded(Some(2)).wire().ends_with(r#""nOrders":100,"nSeededLevels":2}}"#));
}

#[test]
fn a_token_only_variant_carries_its_token_and_the_rest_their_fields() {
    let only = [
        (SpotDeploy::EnableQuoteToken(9), "enableQuoteToken"),
        (SpotDeploy::EnableAlignedQuoteToken(9), "enableAlignedQuoteToken"),
        (SpotDeploy::DisableQuoteToken(9), "disableQuoteToken"),
        (SpotDeploy::DisableAlignedQuoteToken(9), "disableAlignedQuoteToken"),
        (SpotDeploy::EnableFreezePrivilege(9), "enableFreezePrivilege"),
        (SpotDeploy::RevokeFreezePrivilege(9), "revokeFreezePrivilege"),
    ];
    for (action, name) in only {
        assert_eq!(action.wire(), format!(r#"{{"type":"spotDeploy","{name}":{{"token":9}}}}"#));
    }
    assert_eq!(
        SpotDeploy::RegisterSpot { base: 9, quote: 0 }.wire(),
        r#"{"type":"spotDeploy","registerSpot":{"tokens":[9,0]}}"#
    );
    let share = SpotDeploy::SetDeployerTradingFeeShare { token: 9, share: "99.40%".parse().unwrap() };
    assert_eq!(share.wire(), r#"{"type":"spotDeploy","setDeployerTradingFeeShare":{"token":9,"share":"99.4%"}}"#);
    let label = SpotDeploy::SetDeployerLabel("dfu".parse().unwrap());
    assert_eq!(label.wire(), r#"{"type":"spotDeploy","setDeployerLabel":{"label":"dfu"}}"#);
    let freeze = SpotDeploy::FreezeUser { token: 9, user: user(), freeze: true };
    assert_eq!(
        freeze.wire(),
        format!(r#"{{"type":"spotDeploy","freezeUser":{{"token":9,"user":"{USER}","freeze":true}}}}"#)
    );
    let annotation = Annotation::new("meme", "A cat.", Some("PURR".to_owned()), vec!["cat".to_owned()]).unwrap();
    assert_eq!(
        SpotDeploy::SetTokenAnnotation { token: 9, annotation }.wire(),
        r#"{"type":"spotDeploy","setTokenAnnotation":{"token":9,"annotation":{"category":"meme","description":"A cat.","displayName":"PURR","keywords":["cat"]}}}"#
    );
}

#[test]
fn an_evm_link_takes_minus_two_to_eighteen_extra_decimals_and_finalizes_one_of_three_ways() {
    let below = SpotDeploy::request_evm_contract(9, user(), -2).unwrap();
    assert_eq!(
        below.wire(),
        format!(
            r#"{{"type":"spotDeploy","requestEvmContract":{{"token":9,"address":"{USER}","evmExtraWeiDecimals":-2}}}}"#
        )
    );
    assert!(below.msgpack().ends_with(&[0xfe]), "-2 as a negative fixint");
    assert!(SpotDeploy::request_evm_contract(9, user(), 18).is_ok());
    for refused in [-3, 19] {
        assert_eq!(SpotDeploy::request_evm_contract(9, user(), refused), Err(BadDeploy::EvmExtraWeiDecimals(refused)));
    }
    let finalize = |input| FinalizeEvmContract { token: 9, input }.wire();
    assert_eq!(
        finalize(EvmFinalizer::Create { nonce: 4 }),
        r#"{"type":"finalizeEvmContract","token":9,"input":{"create":{"nonce":4}}}"#
    );
    assert_eq!(
        finalize(EvmFinalizer::FirstStorageSlot),
        r#"{"type":"finalizeEvmContract","token":9,"input":"firstStorageSlot"}"#
    );
    assert_eq!(
        finalize(EvmFinalizer::CustomStorageSlot),
        r#"{"type":"finalizeEvmContract","token":9,"input":"customStorageSlot"}"#
    );
}

#[test]
fn a_fee_share_is_a_percentage_from_none_to_all() {
    for (typed, sent) in [("0%", "0%"), ("0.012%", "0.012%"), ("99.40%", "99.4%"), ("100%", "100%"), ("100.0%", "100%")]
    {
        assert_eq!(typed.parse::<FeeShare>().unwrap().to_wire(), sent, "{typed}");
    }
    assert_eq!(FeeShare::ALL, "100%".parse().unwrap());
    for refused in ["100.01%", "-1%", "+1%", "50", "%", "1 %", "abc%"] {
        assert_eq!(refused.parse::<FeeShare>(), Err(BadDeploy::FeeShare(refused.to_owned())), "{refused}");
    }
    assert_eq!(FeeShare::percent(dec("101")), Err(BadDeploy::FeeShare("101%".to_owned())));
}

#[test]
fn a_label_is_two_to_four_lower_case_letters_and_hyperliquidity_is_not_negative() {
    for name in ["ab", "dfu", "abcd"] {
        assert_eq!(name.parse::<DexName>().unwrap().as_str(), name);
    }
    for refused in ["", "a", "abcde", "AB", "a1", "a b"] {
        assert_eq!(refused.parse::<DexName>(), Err(BadDeploy::Name(refused.to_owned())), "{refused:?}");
    }
    assert!(matches!(
        SpotDeploy::register_hyperliquidity(3, dec("-1"), dec("4"), 100, None),
        Err(BadDeploy::OutOfRange { what: "startPx", .. })
    ));
    assert!(matches!(
        SpotDeploy::register_hyperliquidity(3, dec("1"), dec("-4"), 100, None),
        Err(BadDeploy::OutOfRange { what: "orderSz", .. })
    ));
}
