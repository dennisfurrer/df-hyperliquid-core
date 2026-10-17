//! Run as wasm in Node (`just web`), through Node's own `fetch`. `data:` URLs
//! answer without any network; a port nothing listens on is no answer.

use crate::client::{Http, Request, TransportError};
use wasm_bindgen_test::wasm_bindgen_test;

use super::*;

#[wasm_bindgen_test]
async fn a_get_reads_the_status_headers_and_body() {
    let response = Web.send(Request::get(r#"data:application/json,{"ok":true}"#)).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("application/json"));
    assert_eq!(response.text(), r#"{"ok":true}"#);
}

#[wasm_bindgen_test]
async fn a_binary_body_comes_back_byte_for_byte() {
    let response = Web.send(Request::get("data:application/octet-stream;base64,AAH/gA==")).await.unwrap();
    assert_eq!(response.body, vec![0x00, 0x01, 0xff, 0x80]);
}

#[wasm_bindgen_test]
async fn no_answer_is_unreachable() {
    let error = Web.send(Request::post_json("http://127.0.0.1:9/info", "{}".to_owned())).await.unwrap_err();
    assert!(matches!(error, TransportError::Unreachable(_)), "{error:?}");
}

#[wasm_bindgen_test]
fn the_clock_reads_unix_milliseconds() {
    let now = clock();
    assert!(now > 1_790_000_000_000, "after 2026-09: {now}");
    assert!(now < 4_102_444_800_000, "before 2100: {now}");
}
