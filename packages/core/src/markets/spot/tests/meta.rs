//! `spotMeta` and `spotMetaAndAssetCtxs` as the venue answered them on mainnet, and what an order
//! on a pair names from them; `spot/spot_meta_mainnet.json` (2026-10-18) for a deployer's label.
//! Reads through a client are in `tests/mocked/markets`.

use crate::client::MAINNET_USDC as USDC;
use crate::trade::{Order, PlaceOrder, Side, Tif};
use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::*;

/// Live, mainnet, 2026-10-06, trimmed to four tokens and three pairs.
const META: &str = r#"{"tokens":[{"name":"USDC","szDecimals":8,"weiDecimals":8,"index":0,"tokenId":"0x6d1e7cde53ba9467b783cb7c530ce054","isCanonical":true,"evmContract":{"address":"0x6b9e773128f453f5c2c60935ee2de2cbc5390a24","evm_extra_wei_decimals":-2},"fullName":null,"deployerTradingFeeShare":"0.0"},{"name":"PURR","szDecimals":0,"weiDecimals":5,"index":1,"tokenId":"0xc1fb593aeffbeb02f85e0308e9956a90","isCanonical":true,"evmContract":{"address":"0x9b498c3c8a0b8cd8ba1d9851d40d186f1872b44e","evm_extra_wei_decimals":13},"fullName":null,"deployerTradingFeeShare":"0.0"},{"name":"HYPE","szDecimals":2,"weiDecimals":8,"index":150,"tokenId":"0x0d01dc56dcaaca66ad901c959b4011ec","isCanonical":false,"evmContract":null,"fullName":"Hyperliquid","deployerTradingFeeShare":"0.0"},{"name":"UBTC","szDecimals":5,"weiDecimals":10,"index":197,"tokenId":"0x8f254b963e8468305d409b33aa137c67","isCanonical":false,"evmContract":{"address":"0x9fdbda0a5e284c32744d2f17ee5c74b284993463","evm_extra_wei_decimals":-2},"fullName":"Unit Bitcoin","deployerTradingFeeShare":"1.0"}],"universe":[{"tokens":[1,0],"name":"PURR/USDC","index":0,"isCanonical":true},{"tokens":[150,0],"name":"@107","index":107,"isCanonical":false},{"tokens":[197,0],"name":"@142","index":142,"isCanonical":false}]}"#;
/// The same day's contexts for two pairs and one outcome side.
const CTXS: &str = r##"[{"prevDayPx":"0.1647","dayNtlVlm":"2299853.9030900002","markPx":"0.15262","midPx":"0.15272","circulatingSupply":"594784599.3431500196","coin":"PURR/USDC","totalSupply":"594784610.6496700048","dayBaseVlm":"14751907.0"},{"prevDayPx":"93.591","dayNtlVlm":"62219153.2792899907","markPx":"91.882","midPx":"91.887","circulatingSupply":"298463604.5394721627","coin":"@107","totalSupply":"998882240.850471735","dayBaseVlm":"665615.0699999997"},{"prevDayPx":"0.505","dayNtlVlm":"10549.05","markPx":"0.58","midPx":"0.595","circulatingSupply":"936.0","coin":"#88470","totalSupply":"184467440737095.53125","dayBaseVlm":"21010.0"}]"##;

fn meta() -> SpotMeta {
    serde_json::from_str(META).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_token_is_found_by_its_index_name_id_or_wire_name() {
    let meta = meta();
    let hype = meta.token_named("HYPE").unwrap();
    assert_eq!((hype.index, hype.sz_decimals, hype.wei_decimals, hype.evm_contract.as_ref()), (150, 2, 8, None));
    assert_eq!(
        (hype.wire(), hype.deployer_trading_fee_share),
        ("HYPE:0x0d01dc56dcaaca66ad901c959b4011ec".to_owned(), Some(Dec::ZERO))
    );
    let found = [
        meta.token(150),
        meta.token_by_id("0x0D01DC56DCAACA66AD901C959B4011EC"),
        meta.token_by_wire("HYPE:0x0D01DC56DCAACA66AD901C959B4011EC"),
    ];
    assert_eq!(found, [Some(hype); 3], "an id in either case");
    assert_eq!(meta.token_by_wire("HYPE:0x6d1e7cde53ba9467b783cb7c530ce054"), None, "a name with another's id");
    assert_eq!((meta.token_by_wire("HYPE"), meta.token(3)), (None, None));
    let usdc = meta.usdc().unwrap();
    assert_eq!((usdc.index, usdc.wire()), (0, USDC.wire.to_owned()), "agrees with the USDC constant");
    assert_eq!(usdc.evm_contract.as_ref().map(|evm| evm.extra_wei_decimals), Some(-2));
    assert_eq!(meta.token(197).and_then(|ubtc| ubtc.full_name.as_deref()), Some("Unit Bitcoin"));
}

#[test]
fn a_token_reads_the_label_its_deployer_gave_it() {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/spot/spot_meta_mainnet.json"));
    let meta: SpotMeta = serde_json::from_str(text).unwrap();
    let labelled: Vec<(&str, &str)> =
        meta.tokens.iter().filter_map(|token| Some((token.name.as_str(), token.deployer_label.as_deref()?))).collect();
    assert_eq!(labelled, [("RAT", "rrc")]);
}

#[test]
fn a_pair_is_found_by_its_own_index_its_book_name_its_tokens_or_its_asset_id() {
    let meta = meta();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    assert_eq!(
        (hype.pair.name.as_str(), hype.display_name(), hype.grid()),
        ("@107", "HYPE/USDC".to_owned(), Grid::spot(2))
    );
    assert_eq!((hype.asset().map(Asset::id), hype.pair.base(), hype.pair.quote()), (Some(10_107), 150, 0));
    let found = [meta.market(107), meta.market_named("@107"), meta.market_at(Asset::from_id(10_107))];
    assert_eq!(found, [Some(hype); 3]);
    assert_eq!(
        (meta.pair_named("@107"), meta.pair_of(150, 0), meta.pair_of(0, 150)),
        (Some(hype.pair), Some(hype.pair), None),
        "base first"
    );
    let purr = meta.market_named("PURR/USDC").unwrap();
    assert_eq!((purr.pair.index, purr.asset().map(Asset::id), purr.grid()), (0, Some(10_000), Grid::spot(0)));
    let missing =
        [meta.market(1), meta.market_at(Asset::from_id(107)), meta.market_of("HYPE", "PURR"), meta.market_named("@1")];
    assert_eq!(missing, [None; 4], "pair 1 is not in this trimmed answer, and a perp id is no pair");
}

/// A token is worth its pair with USDC's mark. Outcome sides share the contexts, keyed by book name.
#[test]
fn contexts_are_keyed_by_book_name_and_price_a_token_in_usdc() {
    let both: SpotMetaAndAssetCtxs = serde_json::from_str(&format!("[{META},{CTXS}]")).unwrap();
    let hype = both.ctx("@107").unwrap();
    assert_eq!((hype.mark_px, hype.reference()), (dec("91.882"), dec("91.887")));
    assert_eq!(both.ctx("#88470").and_then(|side| side.total_supply), Some(dec("184467440737095.53125")));
    assert_eq!(both.ctx("HYPE/USDC"), None, "the book calls it @107");
    let prices =
        [(150, Some(dec("91.882"))), (1, Some(dec("0.15262"))), (0, Some(Dec::ONE)), (197, None), (9999, None)];
    for (token, price) in prices {
        assert_eq!(both.usdc_price(token), price, "token {token}");
    }
    assert_eq!(
        both.market_ctx(107).map(|(market, ctx)| (market.display_name(), ctx.mark_px)),
        Some(("HYPE/USDC".to_owned(), dec("91.882")))
    );
}

#[test]
fn a_spot_order_goes_out_on_the_base_s_grid() {
    let meta = meta();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    let grid = hype.grid();
    let price = grid.price(dec("91.88765"), Side::Buy).unwrap();
    let order = Order::limit(hype.asset().unwrap(), Side::Buy, price, grid.size(dec("1.239")).unwrap(), Tif::Gtc);
    assert_eq!(
        PlaceOrder::one(order).wire(),
        r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"91.887","s":"1.23","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#
    );
}
