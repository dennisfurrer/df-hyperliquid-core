//! What a node error's code says, per JSON-RPC and the Ethereum nodes, and revert data as viem
//! encodes it (`viem_evm.json`).

use crate::client::evm::fixtures;
use crate::client::evm::rpc::{BlockTag, CallRequest, eth};

use super::*;

#[test]
fn a_node_error_says_its_kind_and_a_revert_its_reason() {
    let error = |code: i64, message: &str| NodeError { code, message: message.to_owned(), data: None };
    for (code, message, kind) in [
        (-32700, "parse error", ErrorKind::Parse),
        (-32600, "invalid request", ErrorKind::InvalidRequest),
        (-32601, "Method not found", ErrorKind::MethodNotFound),
        (-32602, "invalid params", ErrorKind::InvalidParams),
        (-32603, "internal error", ErrorKind::Internal),
        (3, "execution reverted", ErrorKind::Reverted),
        (-32000, "execution reverted: no", ErrorKind::Reverted),
        (-32000, "execution aborted (timeout = 5s)", ErrorKind::Server),
        (-32003, "EVM error: PrecompileError", ErrorKind::Server),
        (-32100, "out of range", ErrorKind::Other),
        (1, "other", ErrorKind::Other),
    ] {
        assert_eq!(error(code, message).kind(), kind, "{code} {message}");
    }
    assert_eq!(error(3, "execution reverted").to_string(), "execution reverted (code 3)");
    let with_data = NodeError { data: Some("0xdead".to_owned()), ..error(3, "execution reverted") };
    assert_eq!(with_data.to_string(), "execution reverted (code 3): 0xdead");

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
    // A message whose string runs past the data is kept as bytes, as is anything unknown.
    let short = [0x08, 0xc3, 0x79, 0xa0, 0x00];
    for data in [&short[..], &[0xde, 0xad, 0xbe, 0xef, 0x01], &[]] {
        assert_eq!(read_revert(data), Revert::Custom(data.to_vec()));
    }
}
