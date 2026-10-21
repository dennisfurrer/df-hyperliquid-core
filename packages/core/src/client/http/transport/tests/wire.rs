//! Requests and responses as the wire carries them: the method, the body, headers found whatever
//! their case, and `Retry-After` in whole seconds.

use super::*;

#[test]
fn requests_and_answers_carry_their_headers_whatever_their_case() {
    let post =
        Request::post_json("https://example.test/info", r#"{"type":"meta"}"#.to_owned()).with_header("X-Key", "k");
    assert_eq!((post.method, post.url.as_str()), (Method::Post, "https://example.test/info"));
    assert_eq!((post.header("content-type"), post.header("x-key")), (Some("application/json"), Some("k")));
    assert_eq!(post.body, br#"{"type":"meta"}"#.to_vec());
    let get = Request::get("https://example.test/stats").with_header("Authorization", "Bearer t");
    assert_eq!((get.method, get.body.len()), (Method::Get, 0));
    assert_eq!(get.headers, [("Authorization".to_owned(), "Bearer t".to_owned())]);

    for (status, retry_after, success, wait) in [
        (429, Some("3"), false, Some(3_000)),
        (503, Some("Wed, 21 Oct 2026 07:28:00 GMT"), false, None),
        (204, None, true, None),
    ] {
        let headers = retry_after.map(|value| vec![("retry-after".to_owned(), value.to_owned())]).unwrap_or_default();
        let answer = Response { status, headers, body: b"null".to_vec() };
        assert_eq!(answer.header("Retry-After"), retry_after);
        assert_eq!((answer.is_success(), answer.retry_after_ms()), (success, wait), "{status}");
        assert_eq!(answer.text(), "null");
    }
}

/// What carries a signature is a post to `/exchange` or a raw HyperEVM transaction, and only
/// that is refused, and only by an `offline` build.
#[test]
fn what_is_signed_is_a_post_to_exchange_or_a_raw_transaction() {
    let raw = r#"{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0x02"]}"#;
    let call = r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[]}"#;
    for (request, signed) in [
        (Request::post_json("https://api.hyperliquid.xyz/exchange", "{}".to_owned()), true),
        (Request::post_json("https://api.hyperliquid.xyz/exchange/", "{}".to_owned()), true),
        (Request::post_json("https://rpc.hyperliquid.xyz/evm", raw.to_owned()), true),
        (Request::post_json("https://api.hyperliquid.xyz/info", "{}".to_owned()), false),
        (Request::post_json("https://rpc.hyperliquid.xyz/evm", call.to_owned()), false),
        (Request::get("https://api.hyperliquid.xyz/exchange"), false),
    ] {
        assert_eq!(request.is_signed(), signed, "{}", request.url);
        let refused = matches!(request.refuse_if_offline(), Err(TransportError::Offline(_)));
        assert_eq!(refused, OFFLINE && signed, "{}", request.url);
    }
}
