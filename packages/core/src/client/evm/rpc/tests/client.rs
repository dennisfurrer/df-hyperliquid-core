//! The client against HyperEVM mainnet's answers, verbatim (`hyperevm_mainnet.json`), and
//! HTTP failures kept apart from the node's refusals.

use crate::client::Response;
use crate::client::evm::fixtures;
use crate::client::evm::records::BlockTransactions;
use crate::client::evm::rpc::ErrorKind;
use crate::test_support::{Wire, block_on, ok};

use super::*;

#[test]
fn the_client_posts_json_to_its_url_and_reads_the_answer() {
    let wire = Wire::answering([ok(&fixtures::answer("eth_chainId", 0))]);
    let rpc = Rpc::hyperevm(wire, Network::Mainnet);
    assert_eq!(rpc.url(), "https://rpc.hyperliquid.xyz/evm");
    assert_eq!(block_on(rpc.chain_id()), Ok(999));
    let sent = rpc.http().bodies();
    assert_eq!(sent, [("https://rpc.hyperliquid.xyz/evm".to_owned(), eth::chain_id().body())]);
    assert_eq!(Rpc::hyperevm(Wire::answering([]), Network::Testnet).url(), "https://rpc.hyperliquid-testnet.xyz/evm");
    let arbitrum = Rpc::new(
        Wire::answering([ok(r#"{"jsonrpc":"2.0","id":1,"result":"0xa4b1"}"#)]),
        "https://arb1.arbitrum.io/rpc",
    );
    assert_eq!(block_on(arbitrum.chain_id()), Ok(Network::Mainnet.arbitrum_chain_id()));
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
    let rpc = Rpc::new(Wire::answering([]), "https://rpc.example/evm");
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
