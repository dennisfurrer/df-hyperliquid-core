//! A spot pair found by its tokens, with its context and its base token's details, and an order on
//! its grid.

use df_hyperliquid_core::markets::spot;
use df_hyperliquid_core::trade::{self, Order, OrderAnswer, PlaceOrder, Side, Tif};
use df_hyperliquid_core::{Client, Network};

use super::{bodies, dec, sent_order};
use crate::support::venue::Venue;
use crate::support::{clock, key};

/// `spotMeta` and `spotMetaAndAssetCtxs` on 2026-10-18, trimmed to five tokens and three pairs, and
/// HYPE's `tokenDetails` on 2026-10-08. HYPE/USDC is pair 107, asset 10107, two size decimals.
#[tokio::test]
async fn hype_is_found_by_its_tokens_priced_by_its_pair_and_bought_on_its_grid() {
    let venue = Venue::mainnet()
        .info("spotMeta", "spot/spot_meta_mainnet.json")
        .info("spotMetaAndAssetCtxs", "spot/spot_meta_and_asset_ctxs_mainnet.json")
        .info("tokenDetails", "spot/token_details_hype_mainnet.json")
        .exchange("order_resting")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let meta = spot::spot_meta(&client).await.unwrap();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    let listed = spot::spot_meta_and_asset_ctxs(&client).await.unwrap();
    let (market, ctx) = listed.market_ctx(hype.pair.index).unwrap();
    let details = spot::token_details(&client, &market.base.token_id).await.unwrap();
    let grid = market.grid();
    let price = grid.price(ctx.reference(), Side::Buy).unwrap();
    let order = Order::limit(market.asset().unwrap(), Side::Buy, price, grid.size(dec("0.2")).unwrap(), Tif::Gtc);
    let accepted = client.send(&PlaceOrder::one(order)).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"spotMeta"}"#,
            r#"{"type":"spotMetaAndAssetCtxs"}"#,
            r#"{"type":"tokenDetails","tokenId":"0x0d01dc56dcaaca66ad901c959b4011ec"}"#,
        ]
    );
    assert_eq!((hype.pair.name.as_str(), hype.display_name(), market), ("@107", "HYPE/USDC".to_owned(), hype));
    assert_eq!(
        (ctx.mark_px, ctx.mid_px, listed.usdc_price(150)),
        (dec("78.594"), Some(dec("78.5945")), Some(dec("78.594")))
    );
    assert_eq!(listed.ctx("#14730").map(|side| side.mark_px), Some(dec("0.602985")), "outcome sides share the list");
    assert_eq!(
        (details.name.as_str(), details.sz_decimals, details.wei_decimals),
        ("HYPE", 2, 8),
        "as spotMeta lists it"
    );
    assert_eq!(
        sent_order(&client),
        r#"{"a":10107,"b":true,"p":"78.594","s":"0.2","r":false,"t":{"limit":{"tif":"Gtc"}}}"#
    );
    let answers = trade::answer::orders(&accepted, 1).unwrap();
    assert!(matches!(answers.as_slice(), [OrderAnswer::Resting { .. }]), "{answers:?}");
}
