//! Requests and responses as the wire carries them: the method, headers found whatever
//! their case, and the body.

use super::*;

#[test]
fn a_request_carries_its_method_headers_and_body() {
    let post =
        Request::post_json("https://example.test/info", r#"{"type":"meta"}"#.to_owned()).with_header("X-Key", "k");
    assert_eq!((post.method, post.url.as_str()), (Method::Post, "https://example.test/info"));
    assert_eq!(post.header("Content-Type"), Some("application/json"));
    assert_eq!(post.header("x-key"), Some("k"), "a header is found whatever its case");
    assert_eq!(post.body, br#"{"type":"meta"}"#.to_vec());
    let get = Request::get("https://example.test/stats").with_header("Authorization", "Bearer t");
    assert_eq!((get.method, get.body.len()), (Method::Get, 0));
    assert_eq!(get.headers, [("Authorization".to_owned(), "Bearer t".to_owned())]);
}

#[test]
fn a_response_reads_its_headers_and_text() {
    let answer =
        Response { status: 429, headers: vec![("retry-after".to_owned(), "3".to_owned())], body: b"null".to_vec() };
    assert_eq!(answer.header("Retry-After"), Some("3"), "a header is found whatever its case");
    assert_eq!(answer.retry_after_ms(), Some(3_000));
    assert!(!answer.is_success());
    assert_eq!(answer.text(), "null");
    let empty = Response { status: 204, headers: Vec::new(), body: Vec::new() };
    assert!(empty.is_success());
    assert_eq!(empty.retry_after_ms(), None);
    let dated =
        Response { headers: vec![("Retry-After".to_owned(), "Wed, 21 Oct 2026 07:28:00 GMT".to_owned())], ..empty };
    assert_eq!(dated.retry_after_ms(), None, "only whole seconds are read");
}
