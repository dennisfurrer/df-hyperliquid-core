//! Against a server on 127.0.0.1 that answers one request with canned bytes
//! and hands back what it was sent. Nothing leaves the machine.

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

use crate::client::{Http, Method, Request, TransportError};

use super::*;

/// Serves one request with `answer`; the request comes back on the channel.
fn serve_once(answer: Vec<u8>) -> (String, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            if let Some(end) = find(&request, b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&request[..end]).to_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .map_or(0, |length| length.trim().parse::<usize>().unwrap());
                if request.len() >= end + 4 + length {
                    break;
                }
            }
            if read == 0 {
                break;
            }
        }
        stream.write_all(&answer).unwrap();
        sent.send(request).unwrap();
    });
    (url, received)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn answer(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
    for header in headers {
        head.push_str(header);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    [head.into_bytes(), body.to_vec()].concat()
}

#[tokio::test]
async fn a_post_carries_its_method_headers_and_body() {
    let (url, received) = serve_once(answer("200 OK", &[], b"{}"));
    let request = Request::post_json(format!("{url}/info"), r#"{"type":"meta"}"#.to_owned()).with_header("X-Key", "k1");
    let response = Native::new().send(request).await.unwrap();
    assert_eq!((response.status, response.text()), (200, "{}".to_owned()));
    let sent = String::from_utf8(received.recv().unwrap()).unwrap();
    assert!(sent.starts_with("POST /info HTTP/1.1\r\n"), "{sent}");
    let lower = sent.to_lowercase();
    assert!(lower.contains("content-type: application/json\r\n"), "{sent}");
    assert!(lower.contains("x-key: k1\r\n"), "{sent}");
    assert!(sent.ends_with(r#"{"type":"meta"}"#), "{sent}");
}

#[tokio::test]
async fn a_get_goes_without_a_body() {
    let (url, received) = serve_once(answer("200 OK", &[], b"[]"));
    let response = Native::new().send(Request::get(format!("{url}/stats?days=7"))).await.unwrap();
    assert_eq!(response.text(), "[]");
    let sent = String::from_utf8(received.recv().unwrap()).unwrap();
    assert!(sent.starts_with("GET /stats?days=7 HTTP/1.1\r\n"), "{sent}");
}

#[tokio::test]
async fn any_status_is_an_answer_with_its_headers() {
    let (url, _received) = serve_once(answer("429 Too Many Requests", &["Retry-After: 7"], b"null"));
    let response = Native::new().send(Request::post_json(url, "{}".to_owned())).await.unwrap();
    assert_eq!(response.status, 429);
    assert_eq!(response.header("retry-after"), Some("7"));
    assert_eq!(response.text(), "null");
}

#[tokio::test]
async fn a_binary_body_comes_back_byte_for_byte() {
    let bytes: Vec<u8> = (0..=255).collect();
    let (url, _received) = serve_once(answer("200 OK", &["content-type: application/octet-stream"], &bytes));
    let response = Native::new().send(Request::get(url)).await.unwrap();
    assert_eq!(response.body, bytes);
}

#[tokio::test]
async fn no_answer_is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = Native::new().send(Request::post_json(url, "{}".to_owned())).await.unwrap_err();
    assert!(matches!(error, TransportError::Unreachable(_)), "{error:?}");
}

#[test]
fn a_request_s_method_maps_to_the_wire_s() {
    assert_eq!(method(Method::Get), reqwest::Method::GET);
    assert_eq!(method(Method::Post), reqwest::Method::POST);
}

#[test]
fn the_clock_reads_unix_milliseconds() {
    let now = clock();
    assert!(now > 1_790_000_000_000, "after 2026-09: {now}");
    assert!(now < 4_102_444_800_000, "before 2100: {now}");
}
