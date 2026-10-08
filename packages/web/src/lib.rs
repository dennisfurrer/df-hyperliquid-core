//! hl-core's wire for the browser and Workers: [`Web`] sends hl-core's
//! requests through the global `fetch`, and [`clock`] is `Date.now()`. Hand
//! both to [`hl_core::Api`]:
//!
//! ```ignore
//! let api = hl_core::Api::new(hl_web::Web, hl_core::Network::Mainnet, hl_web::clock);
//! ```
//!
//! The global `fetch` is the same function in a page, a Web Worker and a
//! Cloudflare Worker, so nothing here asks for a `window`. Hyperliquid's API
//! answers CORS with `*`, so a page calls it directly. Nothing here retries.

use hl_core::{Http, Method, Request, Response, TransportError};
use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, RequestInit};

/// The global `fetch`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Web;

impl Http for Web {
    async fn send(&self, request: Request) -> Result<Response, TransportError> {
        let global = js_sys::global();
        let fetch: Function =
            Reflect::get(&global, &JsValue::from_str("fetch")).map_err(unreachable)?.dyn_into().map_err(unreachable)?;
        let headers = Headers::new().map_err(unreachable)?;
        for (name, value) in &request.headers {
            headers.append(name, value).map_err(unreachable)?;
        }
        let init = RequestInit::new();
        init.set_method(match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
        });
        init.set_headers(&headers);
        if !request.body.is_empty() {
            init.set_body(&Uint8Array::from(request.body.as_slice()));
        }
        let promise: Promise = fetch
            .call2(&global, &JsValue::from_str(&request.url), &init)
            .map_err(unreachable)?
            .dyn_into()
            .map_err(unreachable)?;
        let answer: web_sys::Response =
            JsFuture::from(promise).await.map_err(unreachable)?.dyn_into().map_err(unreachable)?;
        let status = answer.status();
        let headers = headers_of(&answer.headers());
        let buffer = JsFuture::from(answer.array_buffer().map_err(unreachable)?).await.map_err(unreachable)?;
        Ok(Response { status, headers, body: Uint8Array::new(&buffer).to_vec() })
    }
}

/// Every header as a name and a value. Fetch gives names in lower case.
fn headers_of(headers: &Headers) -> Vec<(String, String)> {
    let Ok(Some(entries)) = js_sys::try_iter(headers) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pair: Array = entry.dyn_into().ok()?;
            Some((pair.get(0).as_string()?, pair.get(1).as_string()?))
        })
        .collect()
}

/// No answer: fetch refused before the venue said anything. For `/exchange`
/// that means unknown, never failed.
fn unreachable(error: JsValue) -> TransportError {
    let message = Reflect::get(&error, &JsValue::from_str("message")).ok().and_then(|message| message.as_string());
    TransportError::Unreachable(message.unwrap_or_else(|| "the request did not complete".to_owned()))
}

/// Unix milliseconds from `Date.now()`: the [`hl_core::Clock`] in the
/// browser and Workers, where std has no clock.
#[must_use]
pub fn clock() -> u64 {
    // Whole milliseconds below 2^53, so the cast is exact.
    js_sys::Date::now() as u64
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
#[path = "tests/fetch.rs"]
mod tests;
