//! Requests are checked against what HyperEVM mainnet was sent and accepted
//! (`hyperevm_mainnet.json`, each call's method and params), answers against
//! what it wrote back, verbatim, and the expected values against node's
//! `BigInt` over the same digits.

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{Network, Response};
use serde_json::{Value, json};

use super::*;
use crate::{fixtures, records::BlockTransactions};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// Answers from a queue and keeps every request. Nothing leaves the process.
#[derive(Default)]
struct Wire {
    answers: RefCell<VecDeque<Result<Response, TransportError>>>,
    sent: RefCell<Vec<Request>>,
}

impl Wire {
    fn answering(answers: impl IntoIterator<Item = Result<Response, TransportError>>) -> Self {
        Wire { answers: RefCell::new(answers.into_iter().collect()), sent: RefCell::default() }
    }
}

impl Http for Wire {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        self.sent.borrow_mut().push(request);
        ready(self.answers.borrow_mut().pop_front().unwrap_or(Err(TransportError::Unreachable("dry".to_owned()))))
    }
}

fn ok(text: &str) -> Result<Response, TransportError> {
    Ok(Response { status: 200, headers: Vec::new(), body: text.as_bytes().to_vec() })
}

fn address(text: &str) -> Address {
    fixtures::address(text)
}

/// The body a query sends, read back as JSON, against the call that was captured.
fn same_request<T>(query: &Query<T>, method: &str, nth: usize) {
    let file = fixtures::mainnet();
    let call = file["calls"].as_array().unwrap().iter().filter(|call| call["method"] == method).nth(nth).unwrap();
    let sent: Value = serde_json::from_str(&query.body()).unwrap();
    assert_eq!(sent, json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": call["params"]}), "{method}");
}

#[test]
fn every_request_is_the_one_the_node_accepted() {
    let usdc = address("0xb88339cb7199b77e23db6e890353e22632ba630f");
    let me = address("0x14791697260e4c9a71f18484c9f997b308e59325");
    let send: B256 = "0x1f7c5f72fb847a4ac78c2ca49ed13b031cee23a93d9cd40025623ce973edc768".parse().unwrap();
    same_request(&eth::block_number(), "eth_blockNumber", 0);
    same_request(&eth::chain_id(), "eth_chainId", 0);
    same_request(&eth::transaction(send), "eth_getTransactionByHash", 0);
    same_request(&eth::receipt(send), "eth_getTransactionReceipt", 0);
    same_request(&eth::block(BlockTag::Number(0x2cc1453), false), "eth_getBlockByNumber", 0);
    let raw_actions = LogFilter {
        from_block: Some(BlockTag::Number(0x2cc1440)),
        to_block: Some(BlockTag::Number(0x2cc1460)),
        address: vec![address("0x3333333333333333333333333333333333333333")],
        topics: vec![Some(vec!["0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3".parse().unwrap()])],
        ..LogFilter::default()
    };
    same_request(&eth::logs(&raw_actions), "eth_getLogs", 0);
    same_request(&eth::code(address("0x3333333333333333333333333333333333333333"), BlockTag::Latest), "eth_getCode", 0);
    same_request(&eth::storage_at(usdc, U256::ZERO, BlockTag::Latest), "eth_getStorageAt", 0);
    same_request(
        &eth::balance(address("0x2222222222222222222222222222222222222222"), BlockTag::Latest),
        "eth_getBalance",
        0,
    );
    same_request(
        &eth::transaction_count(address("0x6b9e773128f453f5c2c60935ee2de2cbc5390a24"), BlockTag::Latest),
        "eth_getTransactionCount",
        0,
    );
    same_request(&eth::gas_price(), "eth_gasPrice", 0);
    same_request(&eth::max_priority_fee(), "eth_maxPriorityFeePerGas", 0);
    same_request(&eth::big_block_gas_price(), "eth_bigBlockGasPrice", 0);
    same_request(&eth::using_big_blocks(me), "eth_usingBigBlocks", 0);
    same_request(&eth::fee_history(2, BlockTag::Latest, &[25, 75]), "eth_feeHistory", 0);
    let balance_of =
        hl_core::hex::decode("0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325").unwrap();
    same_request(
        &eth::estimate_gas(&CallRequest { from: Some(me), ..CallRequest::to(usdc, balance_of) }),
        "eth_estimateGas",
        0,
    );
    // A creation's estimate: no `to`.
    let init = hl_core::hex::decode("0x600180600b6000396000f300").unwrap();
    same_request(
        &eth::estimate_gas(&CallRequest { from: Some(me), data: init, ..CallRequest::default() }),
        "eth_estimateGas",
        1,
    );
    same_request(&eth::call(&CallRequest::to(usdc, vec![0x31, 0x3c, 0xe5, 0x67]), BlockTag::Latest), "eth_call", 0);
}

#[test]
fn every_answer_reads_as_the_exact_integer_or_bytes_it_carries() {
    let read = |method: &str, nth: usize| fixtures::answer(method, nth);
    assert_eq!(eth::chain_id().answer(&read("eth_chainId", 0)), Ok(999));
    assert_eq!(eth::block_number().answer(&read("eth_blockNumber", 1)), Ok(0x2dc97bf));
    assert_eq!(
        eth::balance(Address::ZERO, BlockTag::Latest).answer(&read("eth_getBalance", 0)).unwrap().to_string(),
        "947499191167351653726310821"
    );
    assert_eq!(
        eth::transaction_count(Address::ZERO, BlockTag::Latest).answer(&read("eth_getTransactionCount", 0)),
        Ok(0x15ffdf)
    );
    assert_eq!(eth::gas_price().answer(&read("eth_gasPrice", 0)), Ok(U256::from(315_902_611u64)));
    assert_eq!(eth::max_priority_fee().answer(&read("eth_maxPriorityFeePerGas", 0)), Ok(U256::ZERO));
    assert_eq!(eth::big_block_gas_price().answer(&read("eth_bigBlockGasPrice", 0)), Ok(U256::from(100_000_000u64)));
    assert_eq!(eth::using_big_blocks(Address::ZERO).answer(&read("eth_usingBigBlocks", 0)), Ok(false));
    assert_eq!(eth::estimate_gas(&CallRequest::default()).answer(&read("eth_estimateGas", 0)), Ok(0x7b2f));
    assert_eq!(eth::estimate_gas(&CallRequest::default()).answer(&read("eth_estimateGas", 1)), Ok(0xd3a1));
    let decimals = eth::call(&CallRequest::default(), BlockTag::Latest).answer(&read("eth_call", 0)).unwrap();
    assert_eq!(decimals.len(), 32);
    assert_eq!(decimals[31], 6);
    let code = eth::code(Address::ZERO, BlockTag::Latest).answer(&read("eth_getCode", 0)).unwrap();
    assert!(code.windows(4).any(|window| window == [0x17, 0x93, 0x8e, 0x13]), "CoreWriter's code holds sendRawAction");
    let slot =
        eth::storage_at(Address::ZERO, U256::ZERO, BlockTag::Latest).answer(&read("eth_getStorageAt", 0)).unwrap();
    assert_eq!(slot.to_address().unwrap().to_hex(), "0xb5653bf26797d3271dabf31c616fa2b7b216a053");
    assert_eq!(eth::receipt(B256::ZERO).answer(&read("eth_getTransactionReceipt", 1)), Ok(None), "not mined");
}

#[test]
fn fee_history_keeps_its_ratios_as_written() {
    let history =
        eth::fee_history(2, BlockTag::Latest, &[25, 75]).answer(&fixtures::answer("eth_feeHistory", 0)).unwrap();
    let said: Value = serde_json::from_str(&fixtures::answer("eth_feeHistory", 0)).unwrap();
    let said = &said["result"];
    assert_eq!(
        history.oldest_block,
        u64::from_str_radix(fixtures::text(&said["oldestBlock"]).trim_start_matches("0x"), 16).unwrap()
    );
    assert_eq!(history.base_fee_per_gas.len(), 3, "one more than the blocks asked: the next block's");
    assert_eq!(history.base_fee_per_gas[0].to_quantity(), fixtures::text(&said["baseFeePerGas"][0]));
    assert_eq!(history.next_base_fee(), Some(fixtures::text(&said["baseFeePerGas"][2]).parse().unwrap()));
    // The raw text, not an f64's reading of it.
    let raw = fixtures::answer("eth_feeHistory", 0);
    for ratio in &history.gas_used_ratio {
        assert!(
            raw.contains(&format!("{},", ratio.as_str())) || raw.contains(&format!("{}]", ratio.as_str())),
            "{ratio}"
        );
    }
    assert_eq!(history.reward.len(), 2);
    assert_eq!(history.reward[0].len(), 2);
}

#[test]
fn a_refusal_is_a_typed_node_error() {
    let not_a_method = eth::chain_id().answer(&fixtures::answer("eth_notAMethod", 0)).unwrap_err();
    let RpcError::Node { method, error } = &not_a_method else { panic!("{not_a_method:?}") };
    assert_eq!(*method, "eth_chainId");
    assert_eq!(
        (error.code, error.message.as_str(), error.kind()),
        (-32601, "Method not found", ErrorKind::MethodNotFound)
    );
    let reverted =
        eth::call(&CallRequest::default(), BlockTag::Latest).answer(&fixtures::answer("eth_call", 2)).unwrap_err();
    let RpcError::Node { error, .. } = reverted else { panic!() };
    assert_eq!((error.code, error.kind(), error.revert()), (3, ErrorKind::Reverted, None));
    let timeout = eth::logs(&LogFilter::default()).answer(&fixtures::answer("eth_getLogs", 0)).unwrap_err();
    let RpcError::Node { error, .. } = timeout else { panic!() };
    assert_eq!((error.code, error.kind()), (-32000, ErrorKind::Server));
    assert_eq!(error.message, "execution aborted (timeout = 5s)");
    // A precompile asked about an account it has nothing for.
    let file = fixtures::mainnet();
    let borrow =
        file["precompiles"].as_array().unwrap().iter().find(|call| call["name"] == "borrowLendUserState").unwrap();
    let refused =
        eth::call(&CallRequest::default(), BlockTag::Latest).answer(fixtures::text(&borrow["answer"])).unwrap_err();
    let RpcError::Node { error, .. } = refused else { panic!() };
    assert_eq!((error.code, error.message.as_str()), (-32003, "EVM error: PrecompileError"));
    assert!(matches!(eth::chain_id().answer("<html>"), Err(RpcError::Unreadable { method: "eth_chainId", .. })));
    assert!(matches!(eth::chain_id().answer(r#"{"jsonrpc":"2.0","id":1}"#), Err(RpcError::Unreadable { .. })));
    assert!(matches!(
        eth::chain_id().answer(r#"{"jsonrpc":"2.0","id":1,"result":3}"#),
        Err(RpcError::Unreadable { .. })
    ));
}

#[test]
fn a_revert_reads_its_reason() {
    let viem = fixtures::viem();
    for case in viem["reverts"].as_array().unwrap() {
        let data = fixtures::text(&case["data"]);
        let answer = format!(
            r#"{{"jsonrpc":"2.0","id":1,"error":{{"code":3,"message":"execution reverted","data":"{data}"}}}}"#
        );
        let RpcError::Node { error, .. } =
            eth::call(&CallRequest::default(), BlockTag::Latest).answer(&answer).unwrap_err()
        else {
            panic!()
        };
        let expected = match fixtures::text(&case["kind"]) {
            "message" => Revert::Message(fixtures::text(&case["text"]).to_owned()),
            _ => Revert::Panic(fixtures::text(&case["code"]).parse().unwrap()),
        };
        assert_eq!(error.revert(), Some(expected), "{data}");
        assert_eq!(error.data.as_deref(), Some(data));
    }
    assert_eq!(read_revert(&[0xde, 0xad, 0xbe, 0xef, 0x01]), Revert::Custom(vec![0xde, 0xad, 0xbe, 0xef, 0x01]));
    assert_eq!(read_revert(&[]), Revert::Custom(Vec::new()));
}

#[test]
fn a_log_filter_writes_one_address_bare_and_a_wildcard_as_null() {
    let topic: B256 = "0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3".parse().unwrap();
    let filter = LogFilter {
        address: vec![address("0x3333333333333333333333333333333333333333"), Address::ZERO],
        topics: vec![None, Some(vec![topic, B256::ZERO])],
        block_hash: Some(B256::ZERO),
        ..LogFilter::default()
    };
    let sent: Value = serde_json::from_str(&eth::logs(&filter).body()).unwrap();
    assert_eq!(
        sent["params"][0],
        json!({
            "blockHash": B256::ZERO.to_hex(),
            "address": ["0x3333333333333333333333333333333333333333", Address::ZERO.to_hex()],
            "topics": [null, [topic.to_hex(), B256::ZERO.to_hex()]],
        })
    );
    let tags: Vec<String> = [
        BlockTag::Latest,
        BlockTag::Pending,
        BlockTag::Earliest,
        BlockTag::Safe,
        BlockTag::Finalized,
        BlockTag::Number(0),
    ]
    .iter()
    .map(|tag| tag.to_string())
    .collect();
    assert_eq!(tags, ["latest", "pending", "earliest", "safe", "finalized", "0x0"]);
    let sent: Value = serde_json::from_str(&eth::send_raw_transaction(&[0x02, 0xc0]).body()).unwrap();
    assert_eq!(sent["params"], json!(["0x02c0"]));
    let call =
        CallRequest { gas: Some(21_000), value: Some(U256::from(5u8)), ..CallRequest::to(Address::ZERO, Vec::new()) };
    let sent: Value = serde_json::from_str(&eth::call(&call, BlockTag::Pending).body()).unwrap();
    assert_eq!(sent["params"], json!([{"to": Address::ZERO.to_hex(), "gas": "0x5208", "value": "0x5"}, "pending"]));
}

#[test]
fn the_client_posts_json_to_its_url_and_reads_the_answer() {
    let wire = Wire::answering([ok(&fixtures::answer("eth_chainId", 0))]);
    let rpc = Rpc::hyperevm(wire, Network::Mainnet);
    assert_eq!(rpc.url(), "https://rpc.hyperliquid.xyz/evm");
    assert_eq!(block_on(rpc.chain_id()), Ok(999));
    let sent = rpc.http().sent.borrow();
    assert_eq!(sent[0].url, "https://rpc.hyperliquid.xyz/evm");
    assert_eq!(sent[0].method, hl_core::Method::Post);
    assert_eq!(sent[0].header("content-type"), Some("application/json"));
    assert_eq!(String::from_utf8(sent[0].body.clone()).unwrap(), eth::chain_id().body());
    drop(sent);
    assert_eq!(Rpc::hyperevm(Wire::default(), Network::Testnet).url(), "https://rpc.hyperliquid-testnet.xyz/evm");
    let arbitrum = Rpc::new(
        Wire::answering([ok(r#"{"jsonrpc":"2.0","id":1,"result":"0xa4b1"}"#)]),
        "https://arb1.arbitrum.io/rpc",
    );
    assert_eq!(block_on(arbitrum.chain_id()), Ok(crate::chain::ARBITRUM_ONE));
}

#[test]
fn http_failures_are_kept_apart_from_node_refusals() {
    let limited =
        Response { status: 429, headers: vec![("Retry-After".to_owned(), "3".to_owned())], body: b"{}".to_vec() };
    let rpc = Rpc::new(Wire::answering([Ok(limited)]), "https://rpc.example/evm");
    assert_eq!(
        block_on(rpc.block_number()),
        Err(RpcError::Transport(TransportError::RateLimited { retry_after_ms: Some(3_000) }))
    );
    let broken = Response { status: 502, headers: Vec::new(), body: b"bad gateway".to_vec() };
    let rpc = Rpc::new(Wire::answering([Ok(broken)]), "https://rpc.example/evm");
    assert_eq!(
        block_on(rpc.block_number()),
        Err(RpcError::Transport(TransportError::Status { status: 502, body: "bad gateway".to_owned() }))
    );
    let refused = Response {
        status: 400,
        headers: Vec::new(),
        body: br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params"}}"#.to_vec(),
    };
    let rpc = Rpc::new(Wire::answering([Ok(refused)]), "https://rpc.example/evm");
    let Err(RpcError::Node { error, .. }) = block_on(rpc.block_number()) else { panic!() };
    assert_eq!(error.kind(), ErrorKind::InvalidParams);
    let rpc = Rpc::new(Wire::default(), "https://rpc.example/evm");
    assert!(matches!(
        block_on(rpc.send_raw_transaction(&[0x02])),
        Err(RpcError::Transport(TransportError::Unreachable(_)))
    ));
}

#[test]
fn the_client_reads_receipts_blocks_and_logs() {
    let rpc = Rpc::hyperevm(
        Wire::answering([
            ok(&fixtures::answer("eth_getTransactionReceipt", 0)),
            ok(&fixtures::answer("eth_getBlockByNumber", 0)),
            ok(&fixtures::answer("eth_getTransactionByHash", 0)),
            ok(r#"{"jsonrpc":"2.0","id":1,"result":[]}"#),
            ok(
                r#"{"jsonrpc":"2.0","id":1,"result":"0x1f7c5f72fb847a4ac78c2ca49ed13b031cee23a93d9cd40025623ce973edc768"}"#,
            ),
        ]),
        Network::Mainnet,
    );
    let hash: B256 = "0x1f7c5f72fb847a4ac78c2ca49ed13b031cee23a93d9cd40025623ce973edc768".parse().unwrap();
    let receipt = block_on(rpc.receipt(hash)).unwrap().unwrap();
    assert!(receipt.succeeded());
    assert_eq!(receipt.transaction_hash, hash);
    let block = block_on(rpc.block(BlockTag::Number(0x2cc1453), false)).unwrap().unwrap();
    assert_eq!(block.number, 0x2cc1453);
    assert!(matches!(&block.transactions, BlockTransactions::Hashes(hashes) if hashes.contains(&hash)));
    assert_eq!(block_on(rpc.transaction(hash)).unwrap().unwrap().hash, hash);
    assert_eq!(block_on(rpc.logs(&LogFilter::default())), Ok(Vec::new()));
    assert_eq!(block_on(rpc.send_raw_transaction(&[0x02])), Ok(hash));
}
