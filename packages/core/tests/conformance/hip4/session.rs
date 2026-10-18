//! `outcomes.json`: a first HIP-4 session through an outcome ticket, signed by `@nktkas/hyperliquid` 0.24.3 on
//! mainnet: the owner's approvals, then the agent's orders and cancel.

use df_hyperliquid_core::account::{LocalKey, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::markets::outcomes::Coin;
use df_hyperliquid_core::markets::outcomes::px::{Px, Qty};
use df_hyperliquid_core::trade::builder::fee::BuilderFee;
use df_hyperliquid_core::trade::{Builder, Cancel, PlaceOrder, Side, Tif};
use df_hyperliquid_core::utils::sign::user_signed::{NonceKey, UserSigned};
use df_hyperliquid_core::utils::sign::{Field, L1Action};
use df_hyperliquid_core::utils::{Json, OutcomeSide};

use super::{AGENT_KEY, OWNER_KEY, address, list, number, parse, text};

const OUTCOMES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcomes.json"));

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

fn dec(text: &str) -> df_hyperliquid_core::utils::Dec {
    df_hyperliquid_core::utils::Dec::parse(text).unwrap()
}

fn check_signed(name: &str, case: &Json, action: &dyn L1Action, key: &LocalKey, network: Network) {
    let nonce = number(case, "nonce");
    assert_eq!(action.wire(), case.get("action").unwrap().compact(), "{name}: the action");
    let digest = action.digest(nonce, None, network);
    let signature = key.sign_digest(&digest).unwrap();
    let expected = case.get("signature").unwrap();
    assert_eq!(signature.r_hex(), text(expected, "r"), "{name}: r");
    assert_eq!(signature.s_hex(), text(expected, "s"), "{name}: s");
    assert_eq!(u64::from(signature.v()), number(expected, "v"), "{name}: v");
    assert_eq!(recover(&digest, &signature), Ok(key.address()), "{name}");
    // The recorded body has no `vaultAddress`; this crate posts it as null, which the venue reads as none.
    // Everything else is byte for byte.
    let posted = text(case, "body");
    let with_vault = format!("{},\"vaultAddress\":null}}", &posted[..posted.len() - 1]);
    assert_eq!(action.body(nonce, &signature, None), with_vault, "{name}: the body");
}

#[test]
fn a_first_outcome_session_is_reproduced_on_mainnet() {
    let file = parse(OUTCOMES);
    let owner = LocalKey::from_hex(OWNER_KEY).unwrap();
    let agent = LocalKey::from_hex(AGENT_KEY).unwrap();
    assert_eq!(owner.address(), address(text(&file, "owner")));
    assert_eq!(agent.address(), address(text(&file, "agent")));
    let cases = list(&file, "cases");
    assert_eq!(cases.len(), 6);
    let case = |name: &str| cases.iter().find(|case| text(case, "name") == name).unwrap();
    let builder_address = address("0x6967d7acd1ec5f210e9590666498e0cc5d13a843");

    // The approvals, user-signed by the owner.
    let approve = case("approve_agent_atlas");
    let nonce = number(approve, "nonce");
    let agent_approval = UserSigned::new("approveAgent", "HyperliquidTransaction:ApproveAgent", NonceKey::Nonce, nonce)
        .field("agentAddress", Field::Address(agent.address()))
        .field("agentName", Field::String("atlas"));
    let fee_cap = BuilderFee::from_tenths(1_000).percent();
    let fee_nonce = number(case("approve_builder_fee"), "nonce");
    let fee_approval =
        UserSigned::new("approveBuilderFee", "HyperliquidTransaction:ApproveBuilderFee", NonceKey::Nonce, fee_nonce)
            .field("maxFeeRate", Field::String(&fee_cap))
            .field("builder", Field::Address(builder_address));
    for (name, signed) in [("approve_agent_atlas", &agent_approval), ("approve_builder_fee", &fee_approval)] {
        let case = case(name);
        assert_eq!(signed.wire(Network::Mainnet), case.get("action").unwrap().compact(), "{name}");
        let signature = owner.sign_digest(&signed.digest(Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(
            (signature.r_hex(), signature.s_hex()),
            (text(expected, "r").to_owned(), text(expected, "s").to_owned())
        );
        assert_eq!(signed.body(&signature, Network::Mainnet), text(case, "body"), "{name}: the body");
    }

    // The orders, L1 actions the agent signs, every one through the builder.
    let builder = Builder::new(builder_address, BuilderFee::from_tenths(100));
    let yes = Coin::new(1473, OutcomeSide::Yes);
    let no = yes.opposite();
    let bound = yes.market_within(Side::Buy, px("0.6"), dec("0.02"), Qty::new(1)).unwrap().price();
    let ten_dollars = Qty::spend(dec("10"), Px::from_dec(bound).unwrap()).unwrap();
    assert_eq!(ten_dollars, Qty::new(16));
    let buy =
        PlaceOrder::one(yes.market_within(Side::Buy, px("0.6"), dec("0.02"), ten_dollars).unwrap()).through(builder);
    let rest = PlaceOrder::one(no.order(Side::Buy, px("0.385"), Qty::new(30), Tif::Gtc).unwrap()).through(builder);
    let close =
        PlaceOrder::one(yes.market_within(Side::Sell, px("0.58"), dec("0.02"), ten_dollars).unwrap()).through(builder);
    for (name, order) in [("yes_market_buy", &buy), ("no_limit_buy", &rest), ("yes_close", &close)] {
        assert_eq!(order.over_builder_cap(), None, "{name}: 0.1% is under an outcome's 1%");
        check_signed(name, case(name), order, &agent, Network::Mainnet);
    }
    let cancel = Cancel::one(no.asset().unwrap(), 2);
    check_signed("no_cancel", case("no_cancel"), &cancel, &agent, Network::Mainnet);
}
