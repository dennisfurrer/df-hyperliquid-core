//! Every vector below was made by `@nktkas/hyperliquid` 0.33.2 itself, as
//! hip4-tui recorded it: its own action functions, the key below, nonce
//! 1790412105001 and a transport that kept the body. User-signed digests are
//! viem's `hashTypedData` over the SDK's own EIP-712 types; L1 connection ids
//! are the SDK's `createL1ActionHash`. Mainnet rows only. The last two hashes
//! are hip4-tui's, cross-checked against `hip4-ts`.

use hl_core::{Address, BuilderFee, Dec, L1Action, LocalKey, UserAction, digest_of_wallet_json, hex, recover};

use crate::{
    agent::ApproveAgent,
    builder::ApproveBuilderFee,
    mode::{AbstractionMode, AgentSetAbstraction, UserSetAbstraction},
    referral::{BadCode, SetReferrer},
    state::Abstraction,
    sub_account::{CreateSubAccount, SubAccountSpotTransfer, SubAccountTransfer},
};

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_790_412_105_001;
const OWN: &str = "0x14791697260E4c9A71f18484C9f997B308e59325";
const FRIEND: &str = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414";
const HYPE: &str = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";

fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// `(action, posted json, digest, r, s, v)`.
type Signed = (Box<dyn UserAction>, &'static str, &'static str, &'static str, &'static str, u8);

#[test]
fn every_user_signed_approval_is_the_sdk_s_byte_for_byte() {
    let key = key();
    let cases: Vec<Signed> = vec![
        (
            Box::new(ApproveAgent::new(address(FRIEND), "desk", NONCE)),
            r#"{"type":"approveAgent","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","agentAddress":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","agentName":"desk","nonce":1790412105001}"#,
            "0xd676ad87f3f37a39d32e797ce2ba89da0bf4eca73d9c98e2a64a2fec71bf85b5",
            "0x6a1a6cd321a1af0111b66e782714a4b161ac8176fa82f4c486e3d1769f5bc815",
            "0x107a93402d51f74c1df3bc2422dbe23e143247e84770311a29311c1e1b307afe",
            28,
        ),
        (
            Box::new(ApproveBuilderFee::new(
                address("0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704"),
                BuilderFee::from_tenths(100),
                NONCE,
            )),
            r#"{"type":"approveBuilderFee","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","maxFeeRate":"0.1%","builder":"0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704","nonce":1790412105001}"#,
            "0xacfad0d935ae970190a2e80b68c3cc6fe863d6c443edd603422e68936fb24fc0",
            "0x44a09a627e50111bcef2feb06f29d44b81336d8573fa56bb2e9fcd887c6eb717",
            "0x1fb6aa5a81d7e346b36e8ec8336407c533c934faef530b3a130990afd7302b0b",
            28,
        ),
        // hip4-tui's builder at 880 tenths, `{maxFeeRate: "0.88%"}`, which the
        // SDK's `Percent` schema took.
        (
            Box::new(ApproveBuilderFee::new(
                address("0x6967d7AcD1EC5f210E9590666498e0cC5D13a843"),
                BuilderFee::from_tenths(880),
                NONCE,
            )),
            r#"{"type":"approveBuilderFee","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","maxFeeRate":"0.88%","builder":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","nonce":1790412105001}"#,
            "0xad59f180d28d38a190641cbd95ac679eb9e3d20068781a38a1e4aa3b0ed07c01",
            "0x53482b31f09b90b7a956030466b1b119eb4fb4afe78403a8d3c4982e5d8f49b9",
            "0x55bf1ab58c69d99d0551881194bc9a1b308c1ebf8c17a41066e32b20390f6027",
            28,
        ),
        (
            Box::new(UserSetAbstraction::new(address(OWN), AbstractionMode::UnifiedAccount, NONCE)),
            r#"{"type":"userSetAbstraction","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","user":"0x14791697260e4c9a71f18484c9f997b308e59325","abstraction":"unifiedAccount","nonce":1790412105001}"#,
            "0x98ad9b3d0fd3195cd0722d4e84a945b8bc267b00cb04cf30a6c0f228a0ce7743",
            "0xa1a455ef05e9c261662771fdf3e6770c2e0b69bd755d6e56637a5310a9e13e15",
            "0x40c514a0e8ea87adce9f3233db619acfaf2ec6037ab4359cfbe2626a926a4737",
            27,
        ),
    ];
    for (action, json, digest, r, s, v) in cases {
        assert_eq!(action.wire(), json, "the posted action");
        assert_eq!(hex::encode_prefixed(&action.digest()), digest, "{json}: the digest");
        assert_eq!(digest_of_wallet_json(&action.wallet_json()), Ok(action.digest()), "{json}: wallet json");
        let signature = key.sign_digest(&action.digest()).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(recover(&action.digest(), &signature), Ok(key.address()));
        assert_eq!(
            action.body(&signature),
            format!(r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#)
        );
    }
}

/// `(action, posted json, connection id, r, s, v)`.
type L1 = (Box<dyn L1Action>, &'static str, &'static str, &'static str, &'static str, u8);

#[test]
fn every_l1_account_action_hashes_and_signs_as_the_sdk_does() {
    let key = key();
    let cases: Vec<L1> = vec![
        (
            Box::new(AgentSetAbstraction::new(AbstractionMode::UnifiedAccount)),
            r#"{"type":"agentSetAbstraction","abstraction":"u"}"#,
            "0x95dce6a763ef2d03a3c0676975653e9f9a50d21d0d84a0bd8031963755ac2b6e",
            "0xaecc6129cb70a5038fd06b0bce922916269a8aa18cab91c5c2ecef2934d57404",
            "0x4ea79dffc7df259b9d0b9f90096458433e142de2b2f0e6a7d82c7867e63b064c",
            27,
        ),
        (
            Box::new(SubAccountTransfer::deposit(address(FRIEND), dec("5")).unwrap()),
            r#"{"type":"subAccountTransfer","subAccountUser":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","isDeposit":true,"usd":5000000}"#,
            "0x417526f238ce301e19185b8e27065a05653860da13dc9cf83f6e3c215233e517",
            "0x643814229fe0d4a63062c5b22739208cf3f3d5330517532862e3d9c55cb19c17",
            "0x2f19c56fd551f12b52f936a48af534ce6f6729610748ed7dd76e6b4c3bd4c245",
            28,
        ),
        (
            Box::new(SubAccountSpotTransfer::new(address(FRIEND), false, HYPE, dec("2.0")).unwrap()),
            r#"{"type":"subAccountSpotTransfer","subAccountUser":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","isDeposit":false,"token":"HYPE:0x0d01dc56dcaaca66ad901c959b4011ec","amount":"2"}"#,
            "0x8cc2183c2669e33bad291bb2fab75daccd4c04333622a71fc163b0ce703e023b",
            "0x6ea3154fd55d4d71003230ae7e3920f72113bf2080605ba533e330b4323fdeca",
            "0x53bc81017f9b0a9ca8dc56f39fb09c3f7741c2f045f2e364579e9809f7118b08",
            28,
        ),
        (
            Box::new(CreateSubAccount::new("desk two")),
            r#"{"type":"createSubAccount","name":"desk two"}"#,
            "0x177b724675c47ff53996f70bf3efbaf8c29b5b4aa44c51f68aff11c1c1568f74",
            "0xc00068d100fb500f8ce9e91bf4c11a2b6e92f0ebc9a6e4171c182282e59f354b",
            "0x59cef5622e8ac0615470a71e7f4602b779486b5fef50c7527d58db5a7b399169",
            28,
        ),
        (
            Box::new(SetReferrer::new("DFU").unwrap()),
            r#"{"type":"setReferrer","code":"DFU"}"#,
            "0x6020d822959359822facb2e4d90f972a1a45480e1c1ac34a94e1db7c5a165e16",
            "0x5d01b040c4418b288a586b63599802875c52cfa5ff02ab45823f0494c1d19e2f",
            "0x616124723cfe7fff68046f9b13c2f2a688b819d41e33d34d03fecab507cef863",
            28,
        ),
    ];
    for (action, json, id, r, s, v) in cases {
        assert_eq!(action.wire(), json, "the posted action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(NONCE, None)), id, "{json}: the connection id");
        let signature = key.sign_digest(&action.digest(NONCE, None)).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(
            action.body(NONCE, &signature, None),
            format!(
                r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}},"vaultAddress":null}}"#
            )
        );
    }
}

/// hip4-tui's `the_create_subaccount_action_hashes_to_what_the_other_implementation_gets`
/// and its subaccount transfer twin, at nonce 1700000000000.
#[test]
fn subaccount_actions_hash_as_the_other_implementation_does() {
    let sub = address("0x1234567890abcdef1234567890abcdef12345678");
    let nonce = 1_700_000_000_000;
    assert_eq!(
        hex::encode(&CreateSubAccount::new("desk two").connection_id(nonce, None)),
        "78b7bf674f2b129a45b32e7d7ccab4868fe9df798fdf45285f0275b4c34317db"
    );
    let into = SubAccountTransfer::deposit(sub, dec("2.5")).unwrap();
    assert_eq!(
        hex::encode(&into.connection_id(nonce, None)),
        "7c4729feca68abc1060366b2ae5b7742cd968e494eabf95bc051e09ce6f14094"
    );
    let out = SubAccountTransfer::withdraw(sub, dec("2.5")).unwrap();
    assert_ne!(out.connection_id(nonce, None), into.connection_id(nonce, None), "the direction is signed");
    assert_eq!(SubAccountTransfer::from_micros(sub, true, 2_500_000), into);
}

#[test]
fn what_would_be_refused_is_refused_before_signing() {
    assert!(SubAccountTransfer::deposit(address(FRIEND), dec("0.0000001")).is_err(), "finer than a millionth");
    assert!(SubAccountSpotTransfer::new(address(FRIEND), true, HYPE, Dec::ZERO).is_err());
    assert_eq!(SetReferrer::new(""), Err(BadCode(String::new())));
    let long = "x".repeat(21);
    assert_eq!(SetReferrer::new(&long), Err(BadCode(long.clone())));
    assert!(SetReferrer::new(&"x".repeat(20)).is_ok());
    let unnamed = ApproveAgent::new(address(FRIEND), "", NONCE);
    assert!(unnamed.wire().contains(r#""agentName":"","#), "the unnamed agent is the empty name, sent");
    assert_eq!(ApproveBuilderFee::new(address(FRIEND), BuilderFee::from_tenths(88), NONCE).max_fee_rate(), "0.088%");
}

#[test]
fn unified_back_to_standard_is_refused() {
    assert!(AbstractionMode::Disabled.is_refused_from(&Abstraction::UnifiedAccount));
    assert!(!AbstractionMode::UnifiedAccount.is_refused_from(&Abstraction::Disabled));
    assert!(!AbstractionMode::Disabled.is_refused_from(&Abstraction::Default));
    let codes: Vec<(&str, &str)> = AbstractionMode::ALL.iter().map(|mode| (mode.wire(), mode.code())).collect();
    assert_eq!(codes, [("disabled", "i"), ("unifiedAccount", "u"), ("portfolioMargin", "p")]);
    for mode in AbstractionMode::ALL {
        assert_eq!(Abstraction::parse(mode.wire()).wire(), mode.wire(), "a mode set reads back as itself");
    }
}
