//! HyperCore's state read from HyperEVM through its read precompiles: each read goes out as an
//! `eth_call` with the calldata mainnet took (`vectors/hyperevm_mainnet.json`, captured with viem),
//! reads its answer into its type at the scale its units say, and a precompile's refusal stays a
//! typed node error.

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::evm::precompiles::{
    self, OutcomeState, account_margin_summary, bbo, borrow_lend_reserve_state, borrow_lend_user_state,
    core_user_exists, delegations, delegator_summary, l1_block_number, mark_px, oracle_px, outcome_status,
    perp_asset_info, perp_price, position, position2, spot_balance, spot_info, spot_price, spot_px, token_info,
    token_supply, user_vault_equity, withdrawable,
};
use df_hyperliquid_core::client::evm::{ReadError, Rpc, RpcError};
use df_hyperliquid_core::utils::{Address, Dec};
use serde_json::Value;

use crate::support::vector;
use crate::support::venue::Venue;

/// A vault short BTC.
const SHORT: &str = "0x31ca8395cf837de08b24da3f660e77761dfb974b";
const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
/// A depositor in HLP.
const FOLLOWER: &str = "0x03e161499870b0a37549b16a5d33e0582fe1255e";
const DELEGATOR: &str = "0x0a3343b600d81cdaac131342652241cd32e14349";
/// HYPE's system address on HyperEVM.
const FUND: &str = "0xfefefefefefefefefefefefefefefefefefefefe";

fn a(text: &str) -> Address {
    text.parse().unwrap()
}

/// `raw` counted at `places`.
fn at(raw: impl Into<i128>, places: u32) -> Dec {
    Dec::new(raw.into(), places).unwrap()
}

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

#[tokio::test]
async fn every_read_precompile_goes_out_as_mainnet_took_it_and_reads_into_its_type() {
    let file: Value = serde_json::from_str(&vector("hyperevm_mainnet.json")).unwrap();
    let recorded = file["precompiles"].as_array().unwrap();
    let venue = recorded.iter().fold(Venue::mainnet(), |venue, call| {
        venue.rpc_call(call["params"][0]["to"].as_str().unwrap(), call["answer"].as_str().unwrap())
    });
    let rpc = Rpc::hyperevm(venue, Network::Mainnet);

    // What the BTC short holds, at BTC's five size decimals and USDC's six.
    let short = rpc.read(&position(a(SHORT), 0)).await.unwrap();
    assert_eq!((at(short.szi, 5), at(short.entry_ntl, 6)), (dec("-0.73625"), dec("60679.747102")));
    assert_eq!((short.leverage, short.is_isolated), (20, false));
    let fund = rpc.read(&spot_balance(a(FUND), 150)).await.unwrap();
    assert_eq!((at(fund.total, 8), fund.hold), (dec("47916797.41473923"), 0), "HYPE at its eight wei decimals");
    let equity = rpc.read(&user_vault_equity(a(FOLLOWER), a(HLP))).await.unwrap();
    assert_eq!(at(equity.equity, 6), dec("605152.384017"));
    assert_eq!(at(rpc.read(&withdrawable(a(HLP))).await.unwrap(), 6), dec("40484911.632646"));
    let staked = rpc.read(&delegations(a(DELEGATOR))).await.unwrap();
    assert_eq!(staked.iter().map(|one| at(one.amount, 8)).collect::<Vec<_>>(), [dec("6446.56647816")]);
    let summary = rpc.read(&delegator_summary(a(DELEGATOR))).await.unwrap();
    assert_eq!((summary.delegated, summary.n_pending_withdrawals), (staked[0].amount, 0));

    // What BTC and HYPE trade at.
    let mark = rpc.read(&mark_px(0)).await.unwrap();
    let oracle = rpc.read(&oracle_px(0)).await.unwrap();
    assert_eq!((perp_price(mark, 5), perp_price(oracle, 5)), (Some(dec("80782")), Some(dec("80831.8"))));
    let hype = rpc.read(&spot_px(107)).await.unwrap();
    assert_eq!(spot_price(hype, 2), Some(dec("83.191")), "pair 107's base is HYPE, at two size decimals");
    assert_eq!(rpc.read(&l1_block_number()).await, Ok(1_176_514_427));

    // What is listed.
    let btc = rpc.read(&perp_asset_info(0)).await.unwrap();
    assert_eq!((btc.coin.as_str(), btc.sz_decimals, btc.max_leverage, btc.only_isolated), ("BTC", 5, 40, false));
    let pair = rpc.read(&spot_info(107)).await.unwrap();
    assert_eq!((pair.name.as_str(), pair.tokens), ("@107", [150, 0]));
    let token = rpc.read(&token_info(150)).await.unwrap();
    assert_eq!((token.name.as_str(), token.sz_decimals, token.wei_decimals), ("HYPE", 2, 8));
    let supply = rpc.read(&token_supply(1)).await.unwrap();
    assert!(supply.circulating_supply <= supply.total_supply && supply.total_supply <= supply.max_supply);
    let touch = rpc.read(&bbo(0)).await.unwrap();
    assert_eq!((perp_price(touch.bid, 5), perp_price(touch.ask, 5)), (Some(dec("80791")), Some(dec("80792"))));

    let margin = rpc.read(&account_margin_summary(0, a(SHORT))).await.unwrap();
    assert_eq!(at(margin.account_value, 6), dec("3004144.710872"));
    assert_eq!(rpc.read(&core_user_exists(a(HLP))).await, Ok(true));

    // An account the precompile has nothing for is refused, as the node words it.
    let refused = rpc.read(&borrow_lend_user_state(a(FOLLOWER), 0)).await;
    let Err(ReadError::Rpc(RpcError::Node { error, .. })) = refused else { panic!("{refused:?}") };
    assert_eq!((error.code, error.message.as_str()), (-32003, "EVM error: PrecompileError"));
    let reserve = rpc.read(&borrow_lend_reserve_state(0)).await.unwrap();
    assert_eq!((reserve.borrow_yearly_rate_bps, reserve.supply_yearly_rate_bps), (500, 334));

    assert_eq!(rpc.read(&position2(a(SHORT), 0)).await, Ok(short), "the widened sibling reads the same");
    let outcome = rpc.read(&outcome_status(1473)).await.unwrap();
    assert_eq!((outcome.state(), outcome.in_question(), outcome.settled()), (OutcomeState::Active, Some(198), None));

    // In address order, one call each, as the chain took them. Calldata with no bytes goes
    // unsaid, where viem wrote `0x`.
    let calls = rpc.http().calls();
    assert_eq!(calls.len(), precompiles::ALL.len());
    for ((call, recorded), spec) in calls.iter().zip(recorded).zip(&precompiles::ALL) {
        let (sent, asked) = (&call["params"], &recorded["params"]);
        assert_eq!(recorded["name"], spec.name);
        assert_eq!((&sent[0]["to"], &sent[1]), (&asked[0]["to"], &asked[1]), "{}", spec.name);
        assert_eq!(sent[0].get("data").unwrap_or(&Value::from("0x")), &asked[0]["data"], "{}", spec.name);
    }
}
