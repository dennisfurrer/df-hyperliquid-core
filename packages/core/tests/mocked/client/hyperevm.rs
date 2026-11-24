//! HyperEVM through an [`Rpc`] against the fake node, which answers as mainnet did
//! (`vectors/hyperevm_mainnet.json`, each call's method, params and answer verbatim): every typed
//! read goes out as the node took it, a CoreWriter send that landed reads back whole, a CoreWriter
//! order is built, signed and sent, and an HTTP failure stays apart from the node's refusals. The
//! block-level reads replay `vectors/hyperevm_blocks_mainnet.json` the same way.

use df_hyperliquid_core::Network;
use df_hyperliquid_core::client::TransportError;
use df_hyperliquid_core::client::evm::corewriter::{CORE_WRITER, SEND_RAW_ACTION, Tif, raw_action_logs};
use df_hyperliquid_core::client::evm::rpc::BlockTag::{Latest, Number};
use df_hyperliquid_core::client::evm::tx::{Fees, tx_hash, with_headroom};
use df_hyperliquid_core::client::evm::{
    Action, B256, BlockTransactions, CallRequest, ErrorKind, Function, RawAction, Rpc, RpcError, Token, U256,
};
use df_hyperliquid_core::utils::{Address, Dec, hex};
use serde_json::{Value, json};

use crate::support::venue::Venue;
use crate::support::{key, vector};

const USDC: &str = "0xb88339cb7199b77e23db6e890353e22632ba630f";
/// The account the recorded estimates were made for.
const SENDER: &str = "0x14791697260e4c9a71f18484c9f997b308e59325";
/// USDC's system contract: it sends through CoreWriter, and has no `symbol()`.
const USDC_SYSTEM: &str = "0x6b9e773128f453f5c2c60935ee2de2cbc5390a24";
/// A CoreWriter send on mainnet, made through a contract.
const SEND: &str = "0x1f7c5f72fb847a4ac78c2ca49ed13b031cee23a93d9cd40025623ce973edc768";

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

/// The `nth` recorded call of `method`: its params, and its answer as the node wrote it.
fn recorded(method: &str, nth: usize) -> (Value, String) {
    let file: Value = serde_json::from_str(&vector("hyperevm_mainnet.json")).unwrap();
    let calls = file["calls"].as_array().unwrap();
    let call = calls.iter().filter(|call| call["method"] == method).nth(nth);
    let call = call.unwrap_or_else(|| panic!("no {method} #{nth}"));
    (call["params"].clone(), call["answer"].as_str().unwrap().to_owned())
}

fn answer(method: &str, nth: usize) -> String {
    recorded(method, nth).1
}

/// Each call the node took against the recorded `(method, nth)` it should be, in order.
fn went_out_as_recorded(rpc: &Rpc<Venue>, expected: &[(&str, usize)]) {
    let calls = rpc.http().calls();
    assert_eq!(calls.len(), expected.len());
    for (call, &(method, nth)) in calls.iter().zip(expected) {
        let params = recorded(method, nth).0;
        assert_eq!(call, &json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}), "{method}");
    }
}

#[tokio::test]
async fn every_typed_read_goes_out_as_the_node_took_it_and_reads_its_answer() {
    let venue = Venue::mainnet()
        .rpc("eth_chainId", &answer("eth_chainId", 0))
        .rpc("eth_blockNumber", &answer("eth_blockNumber", 1))
        .rpc("eth_getBalance", &answer("eth_getBalance", 0))
        .rpc("eth_getTransactionCount", &answer("eth_getTransactionCount", 0))
        .rpc("eth_gasPrice", &answer("eth_gasPrice", 0))
        .rpc("eth_maxPriorityFeePerGas", &answer("eth_maxPriorityFeePerGas", 0))
        .rpc("eth_bigBlockGasPrice", &answer("eth_bigBlockGasPrice", 0))
        .rpc("eth_usingBigBlocks", &answer("eth_usingBigBlocks", 0))
        .rpc("eth_feeHistory", &answer("eth_feeHistory", 0))
        .rpc_to("eth_estimateGas", USDC, &answer("eth_estimateGas", 0))
        .rpc("eth_estimateGas", &answer("eth_estimateGas", 1))
        .rpc_call(USDC, &answer("eth_call", 0))
        .rpc_call(USDC_SYSTEM, &answer("eth_call", 2))
        .rpc("eth_getCode", &answer("eth_getCode", 0))
        .rpc("eth_getStorageAt", &answer("eth_getStorageAt", 0));
    let rpc = Rpc::hyperevm(venue, Network::Mainnet);
    let (usdc, sender) = (address(USDC), address(SENDER));

    assert_eq!(rpc.url(), "https://rpc.hyperliquid.xyz/evm");
    assert_eq!(rpc.chain_id().await, Ok(Network::Mainnet.evm_chain_id()));
    assert_eq!(rpc.block_number().await, Ok(0x2dc97bf));
    let held = rpc.balance(address("0x2222222222222222222222222222222222222222"), Latest).await.unwrap();
    assert_eq!(held.to_dec(18), Ok(dec("947499191.167351653726310821")), "HYPE at eighteen places");
    assert_eq!(rpc.transaction_count(address(USDC_SYSTEM), Latest).await, Ok(0x15ffdf));
    assert_eq!(rpc.gas_price().await, Ok(U256::from_u64(315_902_611)));
    assert_eq!(rpc.max_priority_fee().await, Ok(U256::ZERO));
    assert_eq!(rpc.big_block_gas_price().await, Ok(U256::from_u64(100_000_000)));
    assert_eq!(rpc.using_big_blocks(sender).await, Ok(false));
    let history = rpc.fee_history(2, Latest, &[25, 75]).await.unwrap();
    assert_eq!(history.next_base_fee(), Some(U256::from_u64(0x10a6_8cc4)), "one past the blocks asked for");
    assert_eq!((history.oldest_block, history.reward.len()), (0x2dc_9854, 2));
    let ratios: Vec<&str> = history.gas_used_ratio.iter().map(|ratio| ratio.as_str()).collect();
    assert_eq!(ratios, ["0.36564966666666665", "0.037172"], "as the node wrote them, not as an f64 reads them");

    let balance_of = Function::parse("balanceOf(address owner) returns (uint256)").unwrap();
    let call = CallRequest {
        from: Some(sender),
        ..CallRequest::to(usdc, balance_of.encode(&[Token::Address(sender)]).unwrap())
    };
    assert_eq!(rpc.estimate_gas(&call).await, Ok(0x7b2f));
    let creation = CallRequest {
        from: Some(sender),
        data: hex::decode("0x600180600b6000396000f300").unwrap(),
        ..CallRequest::default()
    };
    assert_eq!(rpc.estimate_gas(&creation).await, Ok(0xd3a1), "a creation names no `to`");

    let decimals = Function::parse("decimals() view returns (uint8)").unwrap();
    let said = rpc.call(&CallRequest::to(usdc, decimals.encode(&[]).unwrap()), Latest).await.unwrap();
    assert_eq!(decimals.decode_output(&said).unwrap()[0].as_u8(), Some(6));
    let symbol = Function::parse("symbol() view returns (string)").unwrap();
    let refused = rpc.call(&CallRequest::to(address(USDC_SYSTEM), symbol.encode(&[]).unwrap()), Latest).await;
    let Err(RpcError::Node { method: "eth_call", error }) = refused else { panic!("{refused:?}") };
    assert_eq!((error.kind(), error.revert()), (ErrorKind::Reverted, None), "reverted, with no reason given");

    let code = rpc.code(CORE_WRITER, Latest).await.unwrap();
    assert!(code.windows(4).any(|window| window == SEND_RAW_ACTION), "CoreWriter's code holds sendRawAction");
    let slot = rpc.storage_at(usdc, U256::ZERO, Latest).await.unwrap();
    assert_eq!(slot.to_address(), Some(address("0xb5653bf26797d3271dabf31c616fa2b7b216a053")));

    went_out_as_recorded(
        &rpc,
        &[
            ("eth_chainId", 0),
            ("eth_blockNumber", 1),
            ("eth_getBalance", 0),
            ("eth_getTransactionCount", 0),
            ("eth_gasPrice", 0),
            ("eth_maxPriorityFeePerGas", 0),
            ("eth_bigBlockGasPrice", 0),
            ("eth_usingBigBlocks", 0),
            ("eth_feeHistory", 0),
            ("eth_estimateGas", 0),
            ("eth_estimateGas", 1),
            ("eth_call", 0),
            ("eth_call", 2),
            ("eth_getCode", 0),
            ("eth_getStorageAt", 0),
        ],
    );
}

#[tokio::test]
async fn a_corewriter_send_that_landed_reads_back_from_the_chain() {
    let venue = Venue::mainnet()
        .rpc("eth_getTransactionByHash", &answer("eth_getTransactionByHash", 0))
        .rpc("eth_getTransactionReceipt", &answer("eth_getTransactionReceipt", 0))
        .rpc("eth_getBlockByNumber", &answer("eth_getBlockByNumber", 0))
        .rpc("eth_getLogs", &answer("eth_getLogs", 1));
    let rpc = Rpc::hyperevm(venue, Network::Mainnet);
    let hash: B256 = SEND.parse().unwrap();

    let sent = rpc.transaction(hash).await.unwrap().unwrap();
    let (rebuilt, signature) = (sent.to_eip1559().unwrap(), sent.signature().unwrap());
    assert_eq!(tx_hash(&rebuilt.signed(&signature).unwrap()), hash, "rebuilt from the node's fields");
    assert_eq!(rebuilt.signer(&signature), Ok(sent.from), "signed by the sender the node names");

    let receipt = rpc.receipt(hash).await.unwrap().unwrap();
    assert!(receipt.succeeded());
    assert_eq!((receipt.transaction_hash, receipt.from), (hash, sent.from));
    let actions: Vec<RawAction> = receipt.logs.iter().filter_map(|log| RawAction::from_log(log).ok()).collect();
    let [action] = actions.as_slice() else { panic!("one RawAction among the logs: {actions:?}") };
    assert_eq!(action.user, address(USDC_SYSTEM), "CoreWriter acts for the contract that called it");
    assert!(matches!(action.action, Ok(Action::SendAsset { .. })), "{action:?}");

    let block = rpc.block(Number(receipt.block_number.unwrap()), false).await.unwrap().unwrap();
    let BlockTransactions::Hashes(hashes) = &block.transactions else { panic!("hashes were asked for") };
    assert!(hashes.contains(&hash));

    let window = rpc.logs(&raw_action_logs(Number(0x2dc_9c41), Number(0x2dc_9c72))).await.unwrap();
    let read: Vec<RawAction> = window.iter().map(|log| RawAction::from_log(log).unwrap()).collect();
    assert_eq!(read.len(), 12);
    assert!(read.iter().all(|one| one.version == 1 && one.action.is_ok()), "{read:?}");

    went_out_as_recorded(
        &rpc,
        &[
            ("eth_getTransactionByHash", 0),
            ("eth_getTransactionReceipt", 0),
            ("eth_getBlockByNumber", 0),
            ("eth_getLogs", 1),
        ],
    );
}

#[tokio::test]
async fn a_corewriter_order_is_built_signed_sent_and_awaited() {
    let venue = Venue::mainnet()
        .rpc("eth_chainId", &answer("eth_chainId", 0))
        .rpc("eth_getTransactionCount", &answer("eth_getTransactionCount", 0))
        .rpc("eth_feeHistory", &answer("eth_feeHistory", 0))
        .rpc("eth_maxPriorityFeePerGas", &answer("eth_maxPriorityFeePerGas", 0))
        .rpc("eth_estimateGas", &answer("eth_estimateGas", 0))
        .rpc("eth_getTransactionReceipt", &answer("eth_getTransactionReceipt", 1));
    let rpc = Rpc::hyperevm(venue, Network::Mainnet);
    let key = key();
    let me = key.address();

    // Buy 0.001 BTC at 60000, resting.
    let order = Action::limit_order(0, true, dec("60000"), dec("0.001"), false, Tif::Gtc, 0).unwrap();
    let chain = rpc.chain_id().await.unwrap();
    let nonce = rpc.transaction_count(me, Latest).await.unwrap();
    let base = rpc.fee_history(2, Latest, &[25, 75]).await.unwrap().next_base_fee().unwrap();
    let fees = Fees::over_base(base, rpc.max_priority_fee().await.unwrap());
    let gas = rpc.estimate_gas(&CallRequest::of(Some(me), &order.call())).await.unwrap();
    let tx = order.transaction(Network::Mainnet, nonce, with_headroom(gas), fees);
    let raw = tx.sign(&key).unwrap();
    let hash = rpc.send_raw_transaction(&raw).await.unwrap();
    assert_eq!(rpc.receipt(hash).await, Ok(None), "not mined yet");

    assert_eq!((tx.chain_id, chain, tx.nonce, tx.gas), (999, 999, 0x15ffdf, 0x7b2f * 13 / 10));
    assert_eq!((tx.max_fee, tx.max_priority_fee), (U256::from_u64(2 * 0x10a6_8cc4), U256::ZERO));
    assert_eq!((tx.to, tx.value), (Some(CORE_WRITER), U256::ZERO), "value would void the action");
    assert_eq!(&tx.data[..4], SEND_RAW_ACTION);
    assert_eq!(hash, tx_hash(&raw), "the node answers with the transaction's hash");
    let signature = key.sign_digest(&tx.signing_hash().0).unwrap();
    assert_eq!((tx.signed(&signature), tx.signer(&signature)), (Ok(raw.clone()), Ok(me)));

    let calls = rpc.http().calls();
    let params: Vec<&Value> = calls.iter().map(|call| &call["params"]).collect();
    assert_eq!(params[1], &json!([me.to_hex(), "latest"]), "HyperEVM serves latest only");
    assert_eq!(
        params[4],
        &json!([{"from": me.to_hex(), "to": CORE_WRITER.to_hex(), "data": hex::encode_prefixed(&order.calldata())}])
    );
    assert_eq!(params[5], &json!([hex::encode_prefixed(&raw)]));
    assert_eq!(params[6], &json!([hash.to_hex()]));
}

#[tokio::test]
async fn an_http_failure_stays_apart_from_the_node_s_refusal_and_nothing_is_retried() {
    let invalid = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params"}}"#;
    let venue = Venue::mainnet()
        .retry_after("/evm", 3)
        .status("/evm", 502, "bad gateway")
        .status("/evm", 400, invalid)
        .unreachable("/evm")
        .rpc("eth_blockNumber", &answer("eth_blockNumber", 0));
    let rpc = Rpc::hyperevm(venue, Network::Mainnet);

    let limited = TransportError::RateLimited { retry_after_ms: Some(3_000) };
    assert_eq!(rpc.block_number().await, Err(RpcError::Transport(limited)));
    let gateway = TransportError::Status { status: 502, body: "bad gateway".to_owned() };
    assert_eq!(rpc.block_number().await, Err(RpcError::Transport(gateway)));
    let refused = rpc.block_number().await;
    let Err(RpcError::Node { method: "eth_blockNumber", error }) = refused else { panic!("{refused:?}") };
    assert_eq!((error.code, error.kind()), (-32602, ErrorKind::InvalidParams), "a refusal, whatever the status");
    let lost = rpc.send_raw_transaction(&[0x02, 0xc0]).await;
    assert!(matches!(lost, Err(RpcError::Transport(TransportError::Unreachable(_)))), "{lost:?}");
    assert_eq!(rpc.block_number().await, Ok(0x2dc_995b));
    assert_eq!(rpc.http().calls().len(), 5, "each sent once");
}

/// Block 52021440 (`0x31ac8c0`) on mainnet: one transaction and one system transaction.
const BLOCK: &str = "0xf0175bbe952c972292208b8877d3cbe7a2c0cf8399a9ea7eb3c8829e37817b7f";
const HEIGHT: u64 = 0x031a_c8c0;

/// The `nth` recorded block-level call of `method`: its params, and its answer as the node wrote
/// it.
fn block_read(method: &str, nth: usize) -> (Value, String) {
    let file: Value = serde_json::from_str(&vector("hyperevm_blocks_mainnet.json")).unwrap();
    let calls = file["calls"].as_array().unwrap();
    let call = calls.iter().filter(|call| call["method"] == method).nth(nth);
    let call = call.unwrap_or_else(|| panic!("no {method} #{nth}"));
    (call["params"].clone(), call["answer"].as_str().unwrap().to_owned())
}

/// A node that answers each of `reads`, `(method, nth)`, as recorded; every call must go out as
/// the recorded one did, in this order.
fn block_node(reads: &[(&str, usize)]) -> Rpc<Venue> {
    let venue =
        reads.iter().fold(Venue::mainnet(), |venue, &(method, nth)| venue.rpc(method, &block_read(method, nth).1));
    Rpc::hyperevm(venue, Network::Mainnet)
}

fn went_out_as_the_block_reads(rpc: &Rpc<Venue>, expected: &[(&str, usize)]) {
    let calls = rpc.http().calls();
    assert_eq!(calls.len(), expected.len());
    for (call, &(method, nth)) in calls.iter().zip(expected) {
        let params = block_read(method, nth).0;
        assert_eq!(call, &json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}), "{method}");
    }
}

#[tokio::test]
async fn a_block_s_transactions_receipts_and_system_transactions_read_by_number_and_by_hash() {
    let reads = [
        ("net_version", 0),
        ("web3_clientVersion", 0),
        ("eth_syncing", 0),
        ("eth_getBlockByHash", 0),
        ("eth_getBlockReceipts", 0),
        ("eth_getBlockTransactionCountByHash", 0),
        ("eth_getBlockTransactionCountByNumber", 0),
        ("eth_getTransactionByBlockHashAndIndex", 0),
        ("eth_getTransactionByBlockNumberAndIndex", 0),
        ("eth_getSystemTxsByBlockHash", 0),
        ("eth_getSystemTxsByBlockNumber", 0),
    ];
    let rpc = block_node(&reads);
    let hash = BLOCK.parse::<B256>().unwrap();

    assert_eq!(rpc.net_version().await, Ok(999));
    assert_eq!(rpc.client_version().await.as_deref(), Ok("hyperliquid evm Mainnet"));
    assert_eq!(rpc.syncing().await, Ok(false));
    let block = rpc.block_by_hash(hash, false).await.unwrap().unwrap();
    let receipts = rpc.block_receipts(Number(HEIGHT)).await.unwrap().unwrap();
    let by_hash = rpc.block_transaction_count_by_hash(hash).await.unwrap();
    let by_number = rpc.block_transaction_count(Number(HEIGHT)).await.unwrap();
    let first = rpc.transaction_by_block_hash_and_index(hash, 0).await.unwrap().unwrap();
    let same = rpc.transaction_by_block_and_index(Number(HEIGHT), 0).await.unwrap().unwrap();
    let system = rpc.system_transactions_by_hash(hash).await.unwrap().unwrap();
    let same_system = rpc.system_transactions(Number(HEIGHT)).await.unwrap().unwrap();

    went_out_as_the_block_reads(&rpc, &reads);
    assert_eq!((block.number, block.hash), (HEIGHT, hash));
    let BlockTransactions::Hashes(hashes) = &block.transactions else { panic!("{block:?}") };
    assert_eq!(hashes.as_slice(), [first.hash], "the block lists its one transaction, not the system one");
    assert_eq!((by_hash, by_number), (Some(1), Some(1)));
    let [receipt] = receipts.as_slice() else { panic!("{receipts:?}") };
    assert_eq!((receipt.transaction_hash, receipt.succeeded(), receipt.logs.len()), (first.hash, false, 0));
    assert_eq!(first, same);
    assert_eq!((first.kind, first.block_number, first.transaction_index), (2, Some(HEIGHT), Some(0)));
    assert_eq!(system, same_system);
    let [system] = system.as_slice() else { panic!("{system:?}") };
    assert_eq!((system.kind, system.gas_price, system.from), (0, Some(U256::ZERO), address(USDC_SYSTEM)));
    assert_eq!((system.to, system.r, system.signature()), (Some(address(USDC)), Some(U256::ZERO), None));
}

/// The whole block carries its transaction's record; a hash the node does not know and an index
/// past the last read as nothing, not as a refusal.
#[tokio::test]
async fn a_whole_block_reads_its_records_and_what_is_not_there_reads_as_none() {
    let rpc = block_node(&[("eth_getBlockByHash", 1)]);
    let block = rpc.block_by_hash(BLOCK.parse::<B256>().unwrap(), true).await.unwrap().unwrap();
    let BlockTransactions::Full(records) = &block.transactions else { panic!("{block:?}") };
    assert_eq!((records.len(), records[0].nonce), (1, 0x43_e06e));
    went_out_as_the_block_reads(&rpc, &[("eth_getBlockByHash", 1)]);

    let rpc = block_node(&[("eth_getBlockByHash", 2), ("eth_getTransactionByBlockNumberAndIndex", 1)]);
    let unknown = format!("0x{}", "11".repeat(32)).parse::<B256>().unwrap();
    assert_eq!(rpc.block_by_hash(unknown, false).await, Ok(None));
    assert_eq!(rpc.transaction_by_block_and_index(Number(HEIGHT), 5).await, Ok(None));
    went_out_as_the_block_reads(&rpc, &[("eth_getBlockByHash", 2), ("eth_getTransactionByBlockNumberAndIndex", 1)]);
}
