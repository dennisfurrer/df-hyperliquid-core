//! HyperEVM mainnet, read only: the chain, its fees and a balance through an [`Rpc`], and every
//! read precompile through the crate's typed reads. Each answer must read into its type, and its
//! JSON-RPC envelope must have the shape of the recording the mocked suite replays
//! (`vectors/hyperevm_mainnet.json`, and `vectors/hyperevm_blocks_mainnet.json` for the block-level
//! reads). No transaction is ever sent.

use std::cell::RefCell;

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::evm::precompiles::{
    OutcomeState, account_margin_summary, bbo, borrow_lend_reserve_state, borrow_lend_user_state, core_user_exists,
    delegations, delegator_summary, l1_block_number, mark_px, oracle_px, outcome_status, perp_asset_info, perp_price,
    position, position2, spot_balance, spot_info, spot_price, spot_px, token_info, token_supply, user_vault_equity,
    withdrawable,
};
use df_hyperliquid_core::client::evm::rpc::BlockTag::{Latest, Number};
use df_hyperliquid_core::client::evm::{ReadError, Rpc, RpcError, U256};
use df_hyperliquid_core::client::native::Native;
use df_hyperliquid_core::client::{Http, Request, Response, TransportError, USDC_INDEX};
use df_hyperliquid_core::utils::{Address, Dec};
use serde_json::Value;

use crate::shape::added;
use crate::support::vector;

const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
/// A depositor in HLP with nothing lent or borrowed when recorded.
const FOLLOWER: &str = "0x03e161499870b0a37549b16a5d33e0582fe1255e";
/// HLP's leader.
const LEADER: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";
/// The account the recorded staking reads were made for.
const DELEGATOR: &str = "0x0a3343b600d81cdaac131342652241cd32e14349";
/// HYPE's system address, which holds what has crossed to HyperEVM.
const HYPE_SYSTEM: &str = "0x2222222222222222222222222222222222222222";
const BTC: u32 = 0;
const HYPE: u32 = 159;
/// HYPE/USDC.
const HYPE_PAIR: u32 = 107;

/// The real wire, keeping each answer's text.
struct Kept {
    wire: Native,
    answers: RefCell<Vec<String>>,
}

impl Http for Kept {
    async fn send(&self, request: Request) -> Result<Response, TransportError> {
        let answer = self.wire.send(request).await?;
        self.answers.borrow_mut().push(answer.text());
        Ok(answer)
    }
}

fn node() -> Rpc<Kept> {
    Rpc::hyperevm(Kept { wire: Native::new(), answers: RefCell::default() }, Network::Mainnet)
}

fn a(text: &str) -> Address {
    text.parse().unwrap()
}

/// Holds the node's last answer against the recorded answer to the method or the precompile
/// named `recorded`, and fails naming every key path the node has added since.
fn same_shape(rpc: &Rpc<Kept>, recorded: &str) {
    let live: Value = serde_json::from_str(rpc.http().answers.borrow().last().expect("nothing was sent")).unwrap();
    let files: Vec<Value> = ["hyperevm_mainnet.json", "hyperevm_blocks_mainnet.json"]
        .iter()
        .map(|file| serde_json::from_str(&vector(file)).unwrap())
        .collect();
    let found = files
        .iter()
        .flat_map(|file| ["calls", "precompiles"].map(|list| &file[list]))
        .filter_map(Value::as_array)
        .flatten()
        .find(|call| call["method"] == recorded || call["name"] == recorded)
        .unwrap_or_else(|| panic!("no recorded {recorded}"));
    let said: Value = serde_json::from_str(found["answer"].as_str().unwrap()).unwrap();
    let added = added(&said, &live);
    assert!(added.is_empty(), "the node now answers {recorded} with fields it did not have: {added:#?}");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn hyperevm_is_chain_999_and_its_blocks_move_on() {
    let rpc = node();
    assert_eq!(rpc.chain_id().await, Ok(Network::Mainnet.evm_chain_id()));
    same_shape(&rpc, "eth_chainId");
    assert!(rpc.block_number().await.unwrap() > 0x2dc_97bf, "past the recorded height");
    same_shape(&rpc, "eth_blockNumber");
    assert!(rpc.read(&l1_block_number()).await.unwrap() > 1_176_514_427, "and HyperCore's");
    same_shape(&rpc, "l1BlockNumber");
}

/// A block read by its number and by its hash agrees: its transactions, receipts and system
/// transactions. The block is one with a transaction, a few back from the head.
#[tokio::test]
#[ignore = "mainnet"]
async fn a_block_s_transactions_receipts_and_system_transactions_read_by_number_and_by_hash() {
    let rpc = node();
    assert_eq!(rpc.net_version().await, Ok(Network::Mainnet.evm_chain_id()));
    same_shape(&rpc, "net_version");
    assert!(rpc.client_version().await.unwrap().starts_with("hyperliquid evm"));
    assert_eq!(rpc.syncing().await, Ok(false));
    let head = rpc.block_number().await.unwrap();
    let (number, count) = {
        let mut at = head - 5;
        loop {
            let count = rpc.block_transaction_count(Number(at)).await.unwrap().unwrap();
            if count > 0 {
                break (at, count);
            }
            at -= 1;
        }
    };
    same_shape(&rpc, "eth_getBlockTransactionCountByNumber");
    let block = rpc.block(Number(number), false).await.unwrap().unwrap();
    let by_hash = rpc.block_by_hash(block.hash, false).await.unwrap().unwrap();
    same_shape(&rpc, "eth_getBlockByHash");
    assert_eq!(by_hash, block);
    assert_eq!(rpc.block_transaction_count_by_hash(block.hash).await, Ok(Some(count)));
    let receipts = rpc.block_receipts(Number(number)).await.unwrap().unwrap();
    same_shape(&rpc, "eth_getBlockReceipts");
    assert_eq!(u64::try_from(receipts.len()), Ok(count));
    assert_eq!(rpc.block_receipts_by_hash(block.hash).await, Ok(Some(receipts.clone())));
    let first = rpc.transaction_by_block_and_index(Number(number), 0).await.unwrap().unwrap();
    same_shape(&rpc, "eth_getTransactionByBlockNumberAndIndex");
    assert_eq!(first.hash, receipts[0].transaction_hash);
    let by_hash = rpc.transaction_by_block_hash_and_index(block.hash, 0).await.unwrap().unwrap();
    assert_eq!(by_hash.hash, first.hash);
    assert_eq!(rpc.transaction_by_block_and_index(Number(number), count).await, Ok(None), "past the last");
    let system = rpc.system_transactions(Number(number)).await.unwrap().unwrap();
    assert_eq!(rpc.system_transactions_by_hash(block.hash).await, Ok(Some(system.clone())));
    assert!(system.iter().all(|tx| tx.kind == 0 && tx.gas_price == Some(U256::ZERO)), "{system:?}");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn fees_and_a_balance_read_as_exact_integers() {
    let rpc = node();
    let gas_price = rpc.gas_price().await.unwrap();
    same_shape(&rpc, "eth_gasPrice");
    let history = rpc.fee_history(2, Latest, &[25, 75]).await.unwrap();
    same_shape(&rpc, "eth_feeHistory");
    assert_eq!((history.base_fee_per_gas.len(), history.gas_used_ratio.len()), (3, 2));
    assert!(gas_price > U256::ZERO && history.next_base_fee() > Some(U256::ZERO));
    let held = rpc.balance(a(HYPE_SYSTEM), Latest).await.unwrap();
    same_shape(&rpc, "eth_getBalance");
    assert!(held.to_dec(18).unwrap() > Dec::from_int(1_000_000), "{held} wei of HYPE");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn btc_and_hype_prices_read_from_the_precompiles() {
    let rpc = node();
    for (perp, sz_decimals, coin) in [(BTC, 5, "BTC"), (HYPE, 2, "HYPE")] {
        let info = rpc.read(&perp_asset_info(perp)).await.unwrap();
        same_shape(&rpc, "perpAssetInfo");
        assert_eq!((info.coin.as_str(), info.sz_decimals), (coin, sz_decimals));
        let mark = rpc.read(&mark_px(perp)).await.unwrap();
        same_shape(&rpc, "markPx");
        let oracle = rpc.read(&oracle_px(perp)).await.unwrap();
        same_shape(&rpc, "oraclePx");
        let touch = rpc.read(&bbo(perp)).await.unwrap();
        same_shape(&rpc, "bbo");
        let shown = (perp_price(mark, sz_decimals).unwrap(), perp_price(oracle, sz_decimals).unwrap());
        assert!(mark.abs_diff(oracle) * 20 < oracle, "{coin}: mark and oracle within 5%: {shown:?}");
        assert!(touch.bid < touch.ask && touch.bid.abs_diff(mark) * 20 < mark, "{coin}: {touch:?} about {shown:?}");
    }
    let hype = spot_price(rpc.read(&spot_px(HYPE_PAIR)).await.unwrap(), 2).unwrap();
    same_shape(&rpc, "spotPx");
    assert!(hype > Dec::ONE, "HYPE/USDC at {hype}");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_hlp_vault_reads_from_the_precompiles() {
    let rpc = node();
    let hlp = a(HLP);
    assert_eq!(rpc.read(&core_user_exists(hlp)).await, Ok(true));
    same_shape(&rpc, "coreUserExists");
    let usdc = rpc.read(&spot_balance(hlp, u64::from(USDC_INDEX))).await.unwrap();
    same_shape(&rpc, "spotBalance");
    assert!(usdc.hold <= usdc.total, "{usdc:?}");
    let free = rpc.read(&withdrawable(hlp)).await.unwrap();
    same_shape(&rpc, "withdrawable");
    let margin = rpc.read(&account_margin_summary(0, hlp)).await.unwrap();
    same_shape(&rpc, "accountMarginSummary");
    assert!(margin.account_value > 0 && free <= margin.account_value.unsigned_abs(), "{margin:?}, {free} free");
    let btc = rpc.read(&position(hlp, 0)).await.unwrap();
    same_shape(&rpc, "position");
    assert_eq!(rpc.read(&position2(hlp, 0)).await, Ok(btc), "the widened sibling reads the same");
    same_shape(&rpc, "position2");
    rpc.read(&user_vault_equity(a(LEADER), hlp)).await.unwrap();
    same_shape(&rpc, "userVaultEquity");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn listed_tokens_and_pairs_read_from_the_precompiles() {
    let rpc = node();
    let pair = rpc.read(&spot_info(HYPE_PAIR)).await.unwrap();
    same_shape(&rpc, "spotInfo");
    assert_eq!(pair.tokens, [150, 0], "HYPE against USDC");
    let token = rpc.read(&token_info(150)).await.unwrap();
    same_shape(&rpc, "tokenInfo");
    assert_eq!((token.name.as_str(), token.wei_decimals), ("HYPE", 8));
    assert!(token.spots.contains(&u64::from(HYPE_PAIR)), "{:?}", token.spots);
    let supply = rpc.read(&token_supply(150)).await.unwrap();
    same_shape(&rpc, "tokenSupply");
    assert!(supply.circulating_supply <= supply.total_supply && supply.total_supply <= supply.max_supply);
}

#[tokio::test]
#[ignore = "mainnet"]
async fn staking_lending_and_outcomes_read_from_the_precompiles() {
    let rpc = node();
    let staked = rpc.read(&delegations(a(DELEGATOR))).await.unwrap();
    same_shape(&rpc, "delegations");
    let summary = rpc.read(&delegator_summary(a(DELEGATOR))).await.unwrap();
    same_shape(&rpc, "delegatorSummary");
    assert_eq!(summary.delegated, staked.iter().map(|one| one.amount).sum::<u64>());
    let reserve = rpc.read(&borrow_lend_reserve_state(0)).await.unwrap();
    same_shape(&rpc, "borrowLendReserveState");
    assert!(reserve.total_borrowed <= reserve.total_supplied, "{reserve:?}");
    match rpc.read(&borrow_lend_user_state(a(FOLLOWER), 0)).await {
        Ok(state) => assert!(state.borrow.value >= state.borrow.basis, "{state:?}"),
        Err(ReadError::Rpc(RpcError::Node { error, .. })) => {
            assert_eq!((error.code, error.message.as_str()), (-32003, "EVM error: PrecompileError"));
            same_shape(&rpc, "borrowLendUserState");
        }
        Err(other) => panic!("{other}"),
    }
    let outcome = rpc.read(&outcome_status(1473)).await.unwrap();
    same_shape(&rpc, "outcomeStatus");
    assert!(!matches!(outcome.state(), OutcomeState::NeverExisted | OutcomeState::Other(_)), "{outcome:?}");
}
