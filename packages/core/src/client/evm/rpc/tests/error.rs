//! Revert data as viem encodes it (`viem_evm.json`).

use crate::client::evm::fixtures;
use crate::client::evm::rpc::{BlockTag, CallRequest, eth};

use super::*;

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
