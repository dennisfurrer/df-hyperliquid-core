//! The `web` transport compiled to wasm: exports `btcMid()`, which reads BTC's book from mainnet
//! through the global `fetch` and answers its mid as a string. Build it with
//! `cargo build -p df-hyperliquid-core --example web_book --no-default-features --features sign,web --target wasm32-unknown-unknown`,
//! bind it with `wasm-bindgen --target nodejs` (or `--target web` for a page), and call
//! `await btcMid()` from Node 22, a page or a Worker.

#[cfg(target_arch = "wasm32")]
mod wasm {
    use df_hyperliquid_core::client::web::{Web, clock};
    use df_hyperliquid_core::markets::{Precision, book};
    use df_hyperliquid_core::{Client, Network};
    use wasm_bindgen::prelude::{JsError, wasm_bindgen};

    #[wasm_bindgen(js_name = btcMid)]
    pub async fn btc_mid() -> Result<String, JsError> {
        let client = Client::reader(Web, Network::Mainnet, clock);
        let book = book::l2_book(&client, "BTC", Precision::FULL).await?;
        let mid = book.mid().ok_or_else(|| JsError::new("the book is empty"))?;
        Ok(mid.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("web_book is a wasm example: build it with --target wasm32-unknown-unknown");
}
