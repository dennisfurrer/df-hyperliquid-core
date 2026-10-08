use hl_core::{Dec, L1Action, MAINNET_USDC as USDC};
use hl_trade::{PlaceOrder, Side, Tif};

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
fn a_token_is_found_by_index_name_and_wire_name() {
    let meta = meta();
    let hype = meta.token_named("HYPE").unwrap();
    assert_eq!((hype.index, hype.sz_decimals, hype.wei_decimals), (150, 2, 8));
    assert_eq!(hype.wire(), "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec");
    assert_eq!(meta.token(150), Some(hype));
    assert_eq!(meta.token_by_wire("HYPE:0x0D01DC56DCAACA66AD901C959B4011EC"), Some(hype));
    assert_eq!(meta.token_by_wire("HYPE:0x6d1e7cde53ba9467b783cb7c530ce054"), None, "a name with another's id");
    assert_eq!(hype.evm_contract, None);
    let usdc = meta.token(0).unwrap();
    assert_eq!(usdc.wire(), USDC.wire, "agrees with hl-core's USDC");
    assert_eq!(usdc.evm_contract.as_ref().map(|evm| evm.extra_wei_decimals), Some(-2));
    assert_eq!(meta.token(197).and_then(|ubtc| ubtc.full_name.as_deref()), Some("Unit Bitcoin"));
}

#[test]
fn a_pair_is_found_by_its_own_index_its_book_name_or_its_tokens() {
    let meta = meta();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    assert_eq!(hype.pair.name, "@107");
    assert_eq!(hype.display_name(), "HYPE/USDC");
    assert_eq!(hype.asset().unwrap().id(), 10_107);
    assert_eq!(hype.grid(), Grid::spot(2));
    assert_eq!(meta.pair_named("@107"), Some(hype.pair));
    assert_eq!(meta.pair_of(150, 0), Some(hype.pair));
    assert_eq!(meta.pair_of(0, 150), None, "base first");
    let purr = meta.market(0).unwrap();
    assert_eq!(
        (purr.pair.name.as_str(), purr.asset().unwrap().id(), purr.grid()),
        ("PURR/USDC", 10_000, Grid::spot(0))
    );
    assert_eq!(meta.market(1), None, "pair 1 is not in this trimmed answer");
}

#[test]
fn contexts_are_keyed_by_book_name_outcome_sides_included() {
    let both: SpotMetaAndAssetCtxs = serde_json::from_str(&format!("[{META},{CTXS}]")).unwrap();
    let hype = both.ctx("@107").unwrap();
    assert_eq!((hype.mark_px, hype.reference()), (dec("91.882"), dec("91.887")));
    assert_eq!(both.ctx("#88470").and_then(|side| side.total_supply), Some(dec("184467440737095.53125")));
    assert_eq!(both.ctx("HYPE/USDC"), None, "the book calls it @107");
}

/// Every recorded context, read and sent, carries its circulating supply: one without it is
/// refused, not read as a supply unknown.
#[test]
fn a_context_without_its_circulating_supply_is_refused() {
    let without = r#"{"prevDayPx":"0.1647","dayNtlVlm":"2299853.9030900002","markPx":"0.15262","midPx":"0.15272","coin":"PURR/USDC","totalSupply":"594784610.6496700048","dayBaseVlm":"14751907.0"}"#;
    assert!(serde_json::from_str::<SpotCtx>(without).is_err());
}

/// What a balance of each token is worth: its pair with USDC's mark.
#[test]
fn a_token_is_priced_in_usdc_by_its_pair_s_mark() {
    let both: SpotMetaAndAssetCtxs = serde_json::from_str(&format!("[{META},{CTXS}]")).unwrap();
    assert_eq!(both.usdc_price(150), Some(dec("91.882")));
    assert_eq!(both.usdc_price(1), Some(dec("0.15262")));
    assert_eq!(both.usdc_price(0), Some(Dec::ONE), "USDC is one USDC");
    assert_eq!(both.usdc_price(197), None, "UBTC's pair has no context in this trimmed answer");
    assert_eq!(both.usdc_price(9999), None);
    assert_eq!(
        both.market_ctx(107).map(|(market, ctx)| (market.display_name(), ctx.mark_px)),
        Some(("HYPE/USDC".to_owned(), dec("91.882")))
    );
}

#[test]
fn a_market_is_found_by_its_book_name_or_its_asset_id() {
    let meta = meta();
    let hype = meta.market_named("@107").unwrap();
    assert_eq!(hype.display_name(), "HYPE/USDC");
    assert_eq!(meta.market_at(Asset::from_id(10_107)), Some(hype));
    assert_eq!(meta.market_at(Asset::from_id(107)), None, "a perp id is not a pair");
    assert_eq!(meta.market_named("PURR/USDC").map(|purr| purr.pair.index), Some(0));
    assert_eq!(meta.usdc().map(|usdc| usdc.index), Some(0));
    assert_eq!(meta.token_by_id("0x0d01dc56dcaaca66ad901c959b4011ec").map(|hype| hype.name.as_str()), Some("HYPE"));
    assert_eq!(meta.token(150).and_then(|hype| hype.deployer_trading_fee_share), Some(Dec::ZERO));
}

#[test]
fn a_spot_order_goes_out_on_the_base_s_grid() {
    let meta = meta();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    let order = hype.grid().limit(hype.asset().unwrap(), Side::Buy, dec("91.88765"), dec("1.239"), Tif::Gtc).unwrap();
    assert_eq!(
        PlaceOrder::one(order).wire(),
        r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"91.887","s":"1.23","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#
    );
}

#[test]
fn the_reads_name_their_type() {
    assert_eq!(spot_meta_request(), r#"{"type":"spotMeta"}"#);
    assert_eq!(spot_meta_and_asset_ctxs_request(), r#"{"type":"spotMetaAndAssetCtxs"}"#);
}
