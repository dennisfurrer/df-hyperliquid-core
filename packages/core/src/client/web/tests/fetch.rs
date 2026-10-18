//! Run as wasm in Node (`just web`), through Node's own `fetch`. `data:` URLs
//! answer without any network; a server on 127.0.0.1 started from the test
//! (`echo.js`) hands back what it was sent; a port nothing listens on is no
//! answer.

use crate::client::{Http, Request, TransportError};
use js_sys::Promise;
use serde_json::Value;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

use super::*;

#[wasm_bindgen(module = "/fixtures/web/echo.js")]
extern "C" {
    fn echo() -> Promise;
}

/// What the echo server was sent by `request` to its URL, as it read it.
async fn echoed(request: impl FnOnce(&str) -> Request) -> Value {
    let url = JsFuture::from(echo()).await.unwrap().as_string().unwrap();
    let response = Web.send(request(&url)).await.unwrap();
    serde_json::from_slice(&response.body).unwrap()
}

#[wasm_bindgen_test]
async fn a_post_reads_the_status_headers_and_body() {
    let response = Web.send(Request::post_json(r#"data:application/json,{"ok":true}"#, "{}".to_owned())).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("application/json"));
    assert_eq!(response.text(), r#"{"ok":true}"#);
}

#[wasm_bindgen_test]
async fn a_get_reads_the_status_headers_and_body() {
    let response = Web.send(Request::get(r#"data:application/json,{"ok":true}"#)).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("application/json"));
    assert_eq!(response.text(), r#"{"ok":true}"#);
}

#[wasm_bindgen_test]
async fn a_get_goes_with_its_headers_and_without_a_body() {
    let sent = echoed(|url| Request::get(format!("{url}/stats?days=7")).with_header("Authorization", "Bearer t")).await;
    assert_eq!((&sent["method"], &sent["url"]), (&Value::from("GET"), &Value::from("/stats?days=7")));
    assert_eq!(sent["headers"]["authorization"], "Bearer t");
    assert_eq!(sent["headers"].get("content-type"), None);
    assert_eq!(sent["body"], "");
}

#[wasm_bindgen_test]
async fn a_post_carries_its_method_headers_and_body() {
    let sent = echoed(|url| {
        Request::post_json(format!("{url}/info"), r#"{"type":"meta"}"#.to_owned()).with_header("X-Key", "k1")
    })
    .await;
    assert_eq!((&sent["method"], &sent["url"]), (&Value::from("POST"), &Value::from("/info")));
    assert_eq!(sent["headers"]["content-type"], "application/json");
    assert_eq!(sent["headers"]["x-key"], "k1");
    assert_eq!(sent["body"], r#"{"type":"meta"}"#);
}

#[wasm_bindgen_test]
async fn a_binary_body_comes_back_byte_for_byte() {
    let request = Request::post_json("data:application/octet-stream;base64,AAH/gA==", "{}".to_owned());
    let response = Web.send(request).await.unwrap();
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
