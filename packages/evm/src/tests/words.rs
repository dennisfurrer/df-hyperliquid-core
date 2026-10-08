//! CoreWriter in words, as hip4-tui's `studio/readable.rs` writes it: its
//! live mainnet logs and the summaries it pins for them
//! (`hip4_tui_evm.json`), and its cases for every other action.

use hl_core::Address;

use super::*;
use crate::{
    corewriter::{Abstraction, Action, BorrowLendOp, FinalizeVariant, OutcomeOp, RawAction, Tif},
    fixtures,
    records::Log,
};

/// Names as mainnet gives them, cut to what these actions name.
fn mainnet() -> Names {
    let mut names = Names::default();
    names.markets.insert(25, Market { name: "HYPE".to_owned(), spot: false });
    names.markets.insert(10_107, Market { name: "HYPE/USDC".to_owned(), spot: true });
    names.tokens.insert(0, TokenName { name: "USDC".to_owned(), wei_decimals: 8 });
    names.tokens.insert(150, TokenName { name: "HYPE".to_owned(), wei_decimals: 8 });
    names.dexes = vec!["xyz".to_owned(), "flx".to_owned()];
    names
}

/// A logged action, by the name hip4-tui gave its constant.
fn logged(name: &str) -> (Action, String) {
    let hip4 = fixtures::hip4_tui();
    let case = hip4["corewriter"]["words"].as_array().unwrap().iter().find(|case| case["name"] == name).unwrap();
    let log = Log {
        address: crate::corewriter::CORE_WRITER,
        topics: vec![crate::corewriter::RAW_ACTION_TOPIC, crate::u256::B256::from_address(Address::ZERO)],
        data: fixtures::bytes(&case["data"]),
        block_number: None,
        block_hash: None,
        transaction_hash: None,
        transaction_index: None,
        log_index: None,
        removed: false,
    };
    (RawAction::from_log(&log).unwrap().action.unwrap(), fixtures::text(&case["said"]).to_owned())
}

#[test]
fn every_live_log_reads_as_hip4_tui_says_it() {
    let names = mainnet();
    for name in ["LIMIT_ORDER", "SEND_ASSET", "SPOT_SEND", "API_WALLET", "NAMED_WALLET", "CLASS_TRANSFER"] {
        let (action, said) = logged(name);
        assert_eq!(action.describe(&names), said, "{name}");
    }
}

#[test]
fn what_a_network_does_not_list_is_named_by_its_number() {
    let (order, _) = logged("LIMIT_ORDER");
    assert_eq!(order.describe(&Names::default()), "sell 7 asset 25 at 1.52  reduce only  Ioc");
    let Action::LimitOrder { sz, limit_px, cloid, .. } = order else { panic!() };
    let spot =
        Action::LimitOrder { asset: 10_107, is_buy: true, limit_px, sz, reduce_only: false, tif: Tif::Gtc, cloid };
    assert_eq!(spot.describe(&mainnet()), "buy 7 HYPE/USDC at 1.52  Gtc");
    let (send, _) = logged("SEND_ASSET");
    assert_eq!(send.describe(&Names::default()), "29980000000 wei of token 0 to 0x2b5d..fe37  spot to perp");
    let Action::SendAsset { destination, token, wei, source_dex, .. } = send else { panic!() };
    let from_sub = Action::SendAsset {
        destination,
        sub_account: fixtures::address("0x00000000000000000000000000000000000000aa"),
        source_dex,
        destination_dex: 1,
        token,
        wei,
    };
    assert_eq!(from_sub.describe(&mainnet()), "299.8 USDC to 0x2b5d..fe37  spot to xyz perp  from sub 0x0000..00aa");
    let names = mainnet();
    assert_eq!(
        [names.dex(u32::MAX), names.dex(0), names.dex(2), names.dex(9)],
        ["spot", "perp", "flx perp", "dex 9"].map(str::to_owned)
    );
    let odd = Action::LimitOrder {
        asset: 25,
        is_buy: false,
        limit_px: 150_000_000,
        sz: 0,
        reduce_only: false,
        tif: Tif::Other(9),
        cloid: 0,
    };
    assert_eq!(odd.describe(&names), "sell 0 HYPE at 1.5  encodedTif 9");
}

#[test]
fn every_other_action_says_what_it_does_in_its_own_units() {
    let names = mainnet();
    let validator = fixtures::address("0x5ac99df645f3414876c816caa18b2d234024b487");
    let say = |action: Action| action.describe(&names);
    assert_eq!(say(Action::StakingDeposit { wei: 1_000_249_648_235 }), "10002.49648235 HYPE into staking");
    assert_eq!(say(Action::StakingWithdraw { wei: 150_000_000 }), "1.5 HYPE out of staking");
    assert_eq!(
        say(Action::TokenDelegate { validator, wei: 2_500_000_000, is_undelegate: false }),
        "delegate 25 HYPE to 0x5ac9..b487"
    );
    assert_eq!(
        say(Action::TokenDelegate { validator, wei: 2_500_000_000, is_undelegate: true }),
        "undelegate 25 HYPE from 0x5ac9..b487"
    );
    assert_eq!(
        say(Action::VaultTransfer { vault: validator, is_deposit: true, usd: 12_500_000 }),
        "deposit 12.5 USDC into 0x5ac9..b487"
    );
    assert_eq!(
        say(Action::VaultTransfer { vault: validator, is_deposit: false, usd: 12_500_000 }),
        "withdraw 12.5 USDC from 0x5ac9..b487"
    );
    assert_eq!(say(Action::CancelByOid { asset: 25, oid: 91_233 }), "HYPE  oid 91233");
    assert_eq!(say(Action::CancelByCloid { asset: 25, cloid: 255 }), "HYPE  cloid 0x000000000000000000000000000000ff");
    // Tenths of a basis point: 10 is 0.01%.
    assert_eq!(say(Action::ApproveBuilderFee { max_fee_rate: 10, builder: validator }), "up to 0.01% for 0x5ac9..b487");
    assert_eq!(say(Action::ApproveBuilderFee { max_fee_rate: 500, builder: validator }), "up to 0.5% for 0x5ac9..b487");
    assert_eq!(
        say(Action::BorrowLend { operation: BorrowLendOp::Supply, token: 0, wei: 250_000_000 }),
        "Supply 2.5 USDC"
    );
    assert_eq!(
        say(Action::BorrowLend { operation: BorrowLendOp::Withdraw, token: 0, wei: 0 }),
        "Withdraw the most there is of USDC"
    );
    assert_eq!(
        say(Action::SetAbstraction { user: validator, abstraction: Abstraction::UnifiedAccount }),
        "unifiedAccount for 0x5ac9..b487"
    );
    assert_eq!(
        say(Action::FinalizeEvmContract { token: 150, variant: FinalizeVariant::Create, create_nonce: 3 }),
        "HYPE  Create  nonce 3"
    );
    let outcome = |operation, question, outcome, wei| say(Action::Outcome { operation, question, outcome, wei });
    assert_eq!(outcome(OutcomeOp::SplitOutcome, 0, 1121, 100_000_000), "SplitOutcome  outcome 1121  100000000 wei");
    assert_eq!(outcome(OutcomeOp::MergeQuestion, 7, 0, 0), "MergeQuestion  question 7  the most there is");
    assert_eq!(outcome(OutcomeOp::NegateOutcome, 7, 1121, 5), "NegateOutcome  question 7 outcome 1121  5 wei");
    assert_eq!(short(validator), "0x5ac9..b487");
}
