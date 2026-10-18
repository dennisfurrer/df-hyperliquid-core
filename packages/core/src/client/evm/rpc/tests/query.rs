//! Refusals as HyperEVM mainnet wrote them, verbatim (`hyperevm_mainnet.json`), and answers
//! that do not read.

use crate::client::evm::fixtures;
use crate::client::evm::rpc::{BlockTag, CallRequest, ErrorKind, LogFilter, eth};

use super::*;

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
