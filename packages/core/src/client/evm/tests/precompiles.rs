//! The table against `L1Read.sol` as Hyperliquid's docs attach it (2026-10-08);
//! calldata against what viem encoded and HyperEVM mainnet answered; every
//! answer against viem's `decodeAbiParameters` of the same live bytes
//! (`viem_evm.json`), and a few against the info API.

use crate::client::Network;
use crate::test_support::{Wire, block_on, ok};
use crate::utils::Dec;
use serde_json::Value;

use super::*;
use crate::client::evm::fixtures;
use crate::client::evm::rpc::Rpc;

/// Each live call: its `to`, its calldata, and the bytes it answered.
fn live(name: &str) -> (String, String, Vec<u8>) {
    let file = fixtures::mainnet();
    let call = file["precompiles"].as_array().unwrap().iter().find(|call| call["name"] == name).unwrap();
    let answer: Value = serde_json::from_str(fixtures::text(&call["answer"])).unwrap();
    let bytes = answer.get("result").map(fixtures::bytes).unwrap_or_default();
    (fixtures::text(&call["params"][0]["to"]).to_owned(), fixtures::text(&call["params"][0]["data"]).to_owned(), bytes)
}

/// viem's reading of the same answer.
fn viem(name: &str) -> Value {
    let file = fixtures::viem();
    let found = file["precompileAnswers"].as_array().unwrap().iter().find(|one| one["name"] == name).unwrap();
    found["decoded"].clone()
}

fn s(value: &Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        other => fixtures::text(other).to_lowercase(),
    }
}

/// The read's address and calldata are the ones that were sent, and its answer decodes.
fn sent<T>(read: &Read<T>, name: &str) -> T {
    let (to, data, answer) = live(name);
    assert_eq!(read.at.to_hex(), to, "{name}: address");
    assert_eq!(crate::utils::hex::encode_prefixed(&read.data), data, "{name}: calldata");
    read.decode(&answer).unwrap()
}

const SHORT: &str = "0x31ca8395cf837de08b24da3f660e77761dfb974b";
const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
const FOLLOWER: &str = "0x03e161499870b0a37549b16a5d33e0582fe1255e";
const DELEGATOR: &str = "0x0a3343b600d81cdaac131342652241cd32e14349";
const FUND: &str = "0xfefefefefefefefefefefefefefefefefefefefe";

fn a(text: &str) -> crate::utils::Address {
    fixtures::address(text)
}

#[test]
fn the_table_is_l1read_every_address_from_0x800_once() {
    let addresses: Vec<u16> = ALL.iter().map(|one| one.at).collect();
    assert_eq!(addresses, (0x800..=0x814).collect::<Vec<_>>());
    let names: Vec<&str> = ALL.iter().map(|one| one.name).collect();
    assert_eq!(
        names,
        [
            "position",
            "spotBalance",
            "userVaultEquity",
            "withdrawable",
            "delegations",
            "delegatorSummary",
            "markPx",
            "oraclePx",
            "spotPx",
            "l1BlockNumber",
            "perpAssetInfo",
            "spotInfo",
            "tokenInfo",
            "tokenSupply",
            "bbo",
            "accountMarginSummary",
            "coreUserExists",
            "borrowLendUserState",
            "borrowLendReserveState",
            "position2",
            "outcomeStatus",
        ]
    );
    for one in &ALL {
        assert!(crate::client::evm::abi::params(one.args).is_ok(), "{}: args", one.name);
        assert!(crate::client::evm::abi::params(one.returns).is_ok(), "{}: returns", one.name);
        assert_eq!(spec(one.at), Some(one));
    }
    assert_eq!(address(0x80e).to_hex(), "0x000000000000000000000000000000000000080e");
    assert_eq!(spec(0x815), None);
}

#[test]
fn positions_balances_and_equity_read_as_the_chain_wrote_them() {
    let position = sent(&position(a(SHORT), 0), "position");
    let expected = viem("position");
    assert_eq!(position.szi.to_string(), s(&expected["szi"]));
    assert_eq!(position.entry_ntl.to_string(), s(&expected["entryNtl"]));
    assert_eq!(position.isolated_raw_usd.to_string(), s(&expected["isolatedRawUsd"]));
    assert_eq!(position.leverage.to_string(), s(&expected["leverage"]));
    assert_eq!(position.is_isolated.to_string(), s(&expected["isIsolated"]));
    // The info API said -0.73625 BTC for this vault: szi is in BTC's five size decimals.
    assert_eq!(Dec::new(i128::from(position.szi), 5), Dec::parse("-0.73625").ok());
    assert_eq!(sent(&position2(a(SHORT), 0), "position2"), position, "the widened sibling reads the same");

    let balance = sent(&spot_balance(a(FUND), 150), "spotBalance");
    let expected = viem("spotBalance");
    assert_eq!(
        [balance.total, balance.hold, balance.entry_ntl].map(|one| one.to_string()),
        [s(&expected["total"]), s(&expected["hold"]), s(&expected["entryNtl"])]
    );

    let equity = sent(&user_vault_equity(a(FOLLOWER), a(HLP)), "userVaultEquity");
    let expected = viem("userVaultEquity");
    assert_eq!(equity.equity.to_string(), s(&expected["equity"]));
    assert_eq!(equity.locked_until_timestamp.to_string(), s(&expected["lockedUntilTimestamp"]));

    assert_eq!(sent(&withdrawable(a(HLP)), "withdrawable").to_string(), s(&viem("withdrawable")["withdrawable"]));

    let margin = sent(&account_margin_summary(0, a(SHORT)), "accountMarginSummary");
    let expected = viem("accountMarginSummary");
    assert_eq!(
        [
            margin.account_value.to_string(),
            margin.margin_used.to_string(),
            margin.ntl_pos.to_string(),
            margin.raw_usd.to_string()
        ],
        [s(&expected["accountValue"]), s(&expected["marginUsed"]), s(&expected["ntlPos"]), s(&expected["rawUsd"])]
    );
    assert!(sent(&core_user_exists(a(HLP)), "coreUserExists"));
}

#[test]
fn staking_reads_as_the_chain_wrote_it() {
    let delegations = sent(&delegations(a(DELEGATOR)), "delegations");
    let expected = viem("delegations");
    assert_eq!(delegations.len(), expected.as_array().unwrap().len());
    for (one, said) in delegations.iter().zip(expected.as_array().unwrap()) {
        assert_eq!(one.validator.to_hex(), s(&said["validator"]));
        assert_eq!(one.amount.to_string(), s(&said["amount"]));
        assert_eq!(one.locked_until_timestamp.to_string(), s(&said["lockedUntilTimestamp"]));
    }
    // The info API said 6446.56647816 HYPE: amounts are HYPE at eight decimals.
    assert_eq!(Dec::new(i128::from(delegations[0].amount), 8), Dec::parse("6446.56647816").ok());
    let summary = sent(&delegator_summary(a(DELEGATOR)), "delegatorSummary");
    let expected = viem("delegatorSummary");
    assert_eq!(
        [summary.delegated, summary.undelegated, summary.total_pending_withdrawal, summary.n_pending_withdrawals]
            .map(|one| one.to_string()),
        [
            s(&expected["delegated"]),
            s(&expected["undelegated"]),
            s(&expected["totalPendingWithdrawal"]),
            s(&expected["nPendingWithdrawals"])
        ]
    );
}

#[test]
fn prices_and_blocks_read_as_the_chain_wrote_them() {
    let mark = sent(&mark_px(0), "markPx");
    assert_eq!(mark.to_string(), s(&viem("markPx")));
    assert_eq!(sent(&oracle_px(0), "oraclePx").to_string(), s(&viem("oraclePx")));
    let spot = sent(&spot_px(107), "spotPx");
    assert_eq!(spot.to_string(), s(&viem("spotPx")));
    assert_eq!(sent(&l1_block_number(), "l1BlockNumber").to_string(), s(&viem("l1BlockNumber")));
    assert!(l1_block_number().data.is_empty(), "the one precompile with no arguments");
    let bbo = sent(&bbo(0), "bbo");
    assert_eq!([bbo.bid.to_string(), bbo.ask.to_string()], [s(&viem("bbo")["bid"]), s(&viem("bbo")["ask"])]);
    // BTC has five size decimals: a perp price is in 10^(6 - 5).
    assert_eq!(perp_price(mark, 5), Dec::new(i128::from(mark), 1));
    assert_eq!(perp_price(807_518, 5), Dec::parse("80751.8").ok());
    // HYPE, the base of pair 107, has two: a spot price is in 10^(8 - 2).
    assert_eq!(spot_price(83_222_000, 2), Dec::parse("83.222").ok());
    assert_eq!(perp_price(1, 7), None, "no perp has more than six size decimals");
    assert_eq!(spot_price(1, 9), None);
}

#[test]
fn assets_tokens_and_supplies_read_as_the_chain_wrote_them() {
    let info = sent(&perp_asset_info(0), "perpAssetInfo");
    let expected = viem("perpAssetInfo");
    assert_eq!(info.coin, "BTC");
    assert_eq!(
        [
            info.margin_table_id.to_string(),
            info.sz_decimals.to_string(),
            info.max_leverage.to_string(),
            info.only_isolated.to_string()
        ],
        [
            s(&expected["marginTableId"]),
            s(&expected["szDecimals"]),
            s(&expected["maxLeverage"]),
            s(&expected["onlyIsolated"])
        ]
    );
    let pair = sent(&spot_info(107), "spotInfo");
    assert_eq!(pair.name, fixtures::text(&viem("spotInfo")["name"]));
    assert_eq!(
        pair.tokens.map(|one| one.to_string()),
        [s(&viem("spotInfo")["tokens"][0]), s(&viem("spotInfo")["tokens"][1])]
    );

    let token = sent(&token_info(150), "tokenInfo");
    let expected = viem("tokenInfo");
    assert_eq!(token.name, "HYPE");
    let spots: Vec<String> = expected["spots"].as_array().unwrap().iter().map(s).collect();
    assert_eq!(token.spots.iter().map(u64::to_string).collect::<Vec<_>>(), spots);
    assert_eq!(token.deployer_trading_fee_share.to_string(), s(&expected["deployerTradingFeeShare"]));
    assert_eq!(token.deployer.to_hex(), s(&expected["deployer"]));
    assert_eq!(token.evm_contract.to_hex(), s(&expected["evmContract"]));
    assert_eq!(
        [token.sz_decimals.to_string(), token.wei_decimals.to_string(), token.evm_extra_wei_decimals.to_string()],
        [s(&expected["szDecimals"]), s(&expected["weiDecimals"]), s(&expected["evmExtraWeiDecimals"])]
    );

    let supply = sent(&token_supply(1), "tokenSupply");
    let expected = viem("tokenSupply");
    assert_eq!(
        [supply.max_supply, supply.total_supply, supply.circulating_supply, supply.future_emissions]
            .map(|one| one.to_string()),
        [
            s(&expected["maxSupply"]),
            s(&expected["totalSupply"]),
            s(&expected["circulatingSupply"]),
            s(&expected["futureEmissions"])
        ]
    );
    let holders = expected["nonCirculatingUserBalances"].as_array().unwrap();
    assert_eq!(supply.non_circulating_user_balances.len(), holders.len());
    for (one, said) in supply.non_circulating_user_balances.iter().zip(holders) {
        assert_eq!((one.user.to_hex(), one.balance.to_string()), (s(&said["user"]), s(&said["balance"])));
    }
}

#[test]
fn lending_and_outcomes_read_as_the_chain_wrote_them() {
    let reserve = sent(&borrow_lend_reserve_state(0), "borrowLendReserveState");
    let expected = viem("borrowLendReserveState");
    assert_eq!(
        [
            reserve.borrow_yearly_rate_bps,
            reserve.supply_yearly_rate_bps,
            reserve.balance,
            reserve.utilization_bps,
            reserve.oracle_px,
            reserve.ltv_bps,
            reserve.total_supplied,
            reserve.total_borrowed,
        ]
        .map(|one| one.to_string()),
        [
            "borrowYearlyRateBps",
            "supplyYearlyRateBps",
            "balance",
            "utilizationBps",
            "oraclePx",
            "ltvBps",
            "totalSupplied",
            "totalBorrowed"
        ]
        .map(|key| s(&expected[key]))
    );
    // No live account answered one; viem made this.
    let file = fixtures::viem();
    let state =
        borrow_lend_user_state(a(FOLLOWER), 0).decode(&fixtures::bytes(&file["borrowLend"]["encoded"])).unwrap();
    assert_eq!(
        state,
        BorrowLendUserState {
            borrow: BasisAndValue { basis: 1_000_000, value: 1_000_350 },
            supply: BasisAndValue { basis: 0, value: 0 },
        }
    );
    let (_, data, _) = live("borrowLendUserState");
    assert_eq!(crate::utils::hex::encode_prefixed(&borrow_lend_user_state(a(FOLLOWER), 0).data), data);

    let outcome = sent(&outcome_status(1473), "outcomeStatus");
    let expected = viem("outcomeStatus");
    assert_eq!(
        [outcome.status.to_string(), outcome.settled_value.to_string(), outcome.question.to_string()],
        [s(&expected["status"]), s(&expected["settledValue"]), s(&expected["question"])]
    );
    assert_eq!(outcome.state(), OutcomeState::Active);
    assert_eq!(outcome.in_question(), Some(198));
    assert_eq!(outcome.settled(), None, "still trading");
    let settled = OutcomeStatus { status: 2, settled_value: 100_000_000, question: u32::MAX };
    assert_eq!(
        (settled.state(), settled.in_question(), settled.settled()),
        (OutcomeState::Settled, None, Some(Dec::ONE))
    );
    assert_eq!(OutcomeStatus { status: 9, ..settled }.state(), OutcomeState::Other(9));
}

#[test]
fn an_answer_shorter_than_its_struct_is_refused() {
    let (_, _, answer) = live("bbo");
    assert!(bbo(0).decode(&answer[..40]).is_err());
    assert!(position(a(SHORT), 0).decode(&[]).is_err());
}

#[test]
fn the_client_reads_a_precompile_and_keeps_its_refusal_typed() {
    let file = fixtures::mainnet();
    let answer = |name: &str| {
        let call = file["precompiles"].as_array().unwrap().iter().find(|call| call["name"] == name).unwrap();
        fixtures::text(&call["answer"]).to_owned()
    };
    let rpc =
        Rpc::hyperevm(Wire::answering([ok(&answer("bbo")), ok(&answer("borrowLendUserState"))]), Network::Mainnet);
    let read = block_on(rpc.read(&bbo(0))).unwrap();
    assert_eq!(read.bid.to_string(), s(&viem("bbo")["bid"]));
    let sent: Value = serde_json::from_slice(&rpc.http().sent()[0].body).unwrap();
    assert_eq!(
        sent["params"],
        serde_json::json!([{"to": "0x000000000000000000000000000000000000080e", "data": format!("0x{}", "0".repeat(64))}, "latest"])
    );
    let refused = block_on(rpc.read(&borrow_lend_user_state(a(FOLLOWER), 0))).unwrap_err();
    let ReadError::Rpc(crate::client::evm::rpc::RpcError::Node { error, .. }) = refused else { panic!("{refused:?}") };
    assert_eq!(error.message, "EVM error: PrecompileError");
}
