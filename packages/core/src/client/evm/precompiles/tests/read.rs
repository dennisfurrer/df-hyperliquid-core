//! The client against HyperEVM mainnet's answers, verbatim.

use crate::client::Network;
use crate::client::evm::fixtures;
use crate::client::evm::precompiles::support::{FOLLOWER, a, s, viem};
use crate::client::evm::precompiles::{bbo, borrow_lend_user_state};
use crate::test_support::{Wire, block_on, ok};
use serde_json::Value;

use super::*;

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
