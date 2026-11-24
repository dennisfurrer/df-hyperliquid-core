//! A perp found with its context and book, on the main dex and on a HIP-3 dex, and what a HIP-3
//! dex holds and may hold.

use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::markets::{Dex, Precision, book, dexes, mids, perps};
use df_hyperliquid_core::trade::{self, Order, OrderAnswer, PlaceOrder, Side, Tif};
use df_hyperliquid_core::{Client, Network};

use super::{bodies, dec, sent_order};
use crate::support::venue::Venue;
use crate::support::{clock, fixture, key};

/// The main dex's perps with their contexts (2026-10-18), the perps at their cap (2026-10-08) and
/// BTC's book at full precision and at five figures in steps of two (2026-10-08).
#[tokio::test]
async fn btc_is_found_with_its_context_its_cap_and_its_book() {
    let venue = Venue::mainnet()
        .info("metaAndAssetCtxs", "perps/meta_and_asset_ctxs_mainnet.json")
        .info("perpsAtOpenInterestCap", "perps/perps_at_open_interest_cap_mainnet.json")
        .info_where("l2Book", &[("nSigFigs", "5")], &fixture("trade/l2_book_five_figures_step_two.json"))
        .info("l2Book", "trade/l2_book.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let listed = perps::meta_and_asset_ctxs(&client, Dex::Main).await.unwrap().unwrap();
    let capped = perps::perps_at_open_interest_cap(&client, Dex::Main).await.unwrap();
    let full = book::l2_book(&client, "BTC", Precision::FULL).await.unwrap();
    let coarse = Precision::figures(5).and_then(|five| five.step(2)).unwrap();
    let stepped = book::l2_book(&client, "BTC", coarse).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"metaAndAssetCtxs"}"#,
            r#"{"type":"perpsAtOpenInterestCap"}"#,
            r#"{"type":"l2Book","coin":"BTC"}"#,
            r#"{"type":"l2Book","coin":"BTC","nSigFigs":5,"mantissa":2}"#,
        ]
    );
    let (asset, btc, ctx) = listed.find(0, "BTC").unwrap();
    assert_eq!((asset.id(), btc.sz_decimals, btc.max_leverage), (0, 5, 40));
    assert_eq!((ctx.mark_px, ctx.reference(), ctx.day_change()), (dec("81885.4"), dec("81867.5"), Some(dec("0.51"))));
    let table = listed.meta.table_of(btc).unwrap();
    assert_eq!((table.max_leverage_at(dec("1000000")), table.max_leverage_at(dec("200000000"))), (Some(40), Some(20)));
    assert!(!capped.iter().any(|coin| coin == "BTC"));
    let (_, ftm, quiet) = listed.find(0, "FTM").unwrap();
    assert!(capped.contains(&ftm.name) && ftm.is_delisted && quiet.mid_px.is_none(), "at its cap, delisted, no book");
    assert_eq!((full.mid(), full.spread), (Some(dec("80858.5")), None));
    assert_eq!((stepped.touch(), stepped.spread), ((Some(dec("80858")), Some(dec("80860"))), Some(dec("1"))));
}

/// `para:10Y` is perp 15 of `para`, the eighth dex: asset 180015, the id the order signs.
#[tokio::test]
async fn a_hip3_perp_is_found_by_its_coin_and_ordered_on_its_own_dex() {
    let venue = Venue::mainnet()
        .info("perpDexs", "hip3/perp_dexs_mainnet.json")
        .info("allPerpMetas", "hip3/all_perp_metas_mainnet.json")
        .info_where("metaAndAssetCtxs", &[("dex", "para")], &fixture("hip3/meta_and_asset_ctxs_para_mainnet.json"))
        .info_where("l2Book", &[("coin", "para:10Y")], &fixture("hip3/l2_book_para_10y_mainnet.json"))
        .exchange("order_resting")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let listing = dexes::dexes(&client).await.unwrap();
    let (asset, perp) = listing.find("para:10Y").unwrap();
    let (place, para, _) = listing.dex("para").unwrap();
    let listed = perps::meta_and_asset_ctxs(&client, Dex::of_coin(&perp.name)).await.unwrap().unwrap();
    let ctx = listed.ctx(place, asset).unwrap();
    let read = book::l2_book(&client, &perp.name, Precision::FULL).await.unwrap();
    let (bid, _) = read.touch();
    let price = perp.grid().price(bid.unwrap(), Side::Buy).unwrap();
    let order = Order::limit(asset, Side::Buy, price, perp.grid().size(dec("10")).unwrap(), Tif::Alo);
    let accepted = client.send(&PlaceOrder::one(order)).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"perpDexs"}"#,
            r#"{"type":"allPerpMetas"}"#,
            r#"{"type":"metaAndAssetCtxs","dex":"para"}"#,
            r#"{"type":"l2Book","coin":"para:10Y"}"#,
        ]
    );
    assert_eq!((asset.id(), place, perp.sz_decimals, perp.max_leverage), (180_015, 8, 1, 20));
    assert_eq!(
        (para.funding_clamp(&perp.name), ctx.mark_px, ctx.reference()),
        (Some(dec("0.005")), dec("5.167"), dec("5.1675"))
    );
    assert_eq!(
        sent_order(&client),
        r#"{"a":180015,"b":true,"p":"5.1269","s":"10","r":false,"t":{"limit":{"tif":"Alo"}}}"#
    );
    assert_eq!(client.http().actions()[0].signer, Some(key().address()));
    let answers = trade::answer::orders(&accepted, 1).unwrap();
    assert!(matches!(answers.as_slice(), [OrderAnswer::Resting { oid: 77_738_308, .. }]), "{answers:?}");
}

/// `xyz`'s limits and status (2026-09-26) and mids (2026-10-08). The main dex has no limits, and a
/// dex the venue does not know has no meta: both `null`.
#[tokio::test]
async fn a_hip3_dex_s_limits_status_and_mids_are_read_by_its_name() {
    let venue = Venue::mainnet()
        .info_where("perpDexLimits", &[("dex", "xyz")], &fixture("hip3/perp_dex_limits_xyz_mainnet.json"))
        .info_where("perpDexLimits", &[("dex", "")], "null")
        .info("perpDexStatus", "hip3/perp_dex_status_xyz_mainnet.json")
        .info("allMids", "trade/all_mids_xyz.json")
        .info_text("perpsAtOpenInterestCap", "[]")
        .info_text("meta", "null")
        .info_text("metaAndAssetCtxs", "null");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let xyz = Dex::Hip3("xyz");

    let limits = dexes::perp_dex_limits(&client, xyz).await.unwrap().unwrap();
    let status = dexes::perp_dex_status(&client, xyz).await.unwrap().unwrap();
    let prices = mids::all_mids(&client, xyz).await.unwrap();
    let capped = perps::perps_at_open_interest_cap(&client, xyz).await.unwrap();
    let main = dexes::perp_dex_limits(&client, Dex::Main).await.unwrap();
    let unknown = Dex::Hip3("zzzz");
    let meta = perps::meta(&client, unknown).await.unwrap();
    let listed = perps::meta_and_asset_ctxs(&client, unknown).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"perpDexLimits","dex":"xyz"}"#,
            r#"{"type":"perpDexStatus","dex":"xyz"}"#,
            r#"{"type":"allMids","dex":"xyz"}"#,
            r#"{"type":"perpsAtOpenInterestCap","dex":"xyz"}"#,
            r#"{"type":"perpDexLimits","dex":""}"#,
            r#"{"type":"meta","dex":"zzzz"}"#,
            r#"{"type":"metaAndAssetCtxs","dex":"zzzz"}"#,
        ]
    );
    assert_eq!((limits.coin_cap("xyz:AAPL"), prices.get("xyz:AAPL")), (Some(dec("200000000")), Some(dec("338.28"))));
    assert_eq!((limits.total_oi_cap, status.total_net_deposit), (dec("10000000000"), dec("1091195769.8109180927")));
    assert!(capped.is_empty());
    assert_eq!((main, meta, listed), (None, None, None));
}

/// The venue lists a dex between the two reads: `allPerpMetas` comes back out of line with
/// `perpDexs`, and an asset id counted against it would sign for another market.
#[tokio::test]
async fn a_dex_list_out_of_line_with_its_metas_is_unreadable() {
    let mut metas: serde_json::Value = serde_json::from_str(&fixture("hip3/all_perp_metas_mainnet.json")).unwrap();
    metas.as_array_mut().unwrap().swap(1, 2);
    let venue =
        Venue::mainnet().info("perpDexs", "hip3/perp_dexs_mainnet.json").info_text("allPerpMetas", &metas.to_string());
    let client = Client::reader(venue, Network::Mainnet, clock);

    let refused = dexes::dexes(&client).await.unwrap_err();

    assert_eq!(bodies(&client), [r#"{"type":"perpDexs"}"#, r#"{"type":"allPerpMetas"}"#]);
    assert_eq!(refused, ApiError::Unreadable { path: "/info", detail: "flx:TSLA is listed under dex xyz".to_owned() });
}

/// Every HIP-3 perp's concise annotation, then one perp's whole annotation, and a main dex perp's,
/// which has none (2026-11-24).
#[tokio::test]
async fn a_hip3_perp_s_annotation_is_read_and_a_main_dex_perp_has_none() {
    let venue = Venue::mainnet()
        .info("perpConciseAnnotations", "hip3/perp_concise_annotations_mainnet.json")
        .info_where("perpAnnotation", &[("coin", "xyz:XYZ100")], &fixture("hip3/perp_annotation_xyz100_mainnet.json"))
        .info("perpAnnotation", "hip3/perp_annotation_btc_mainnet.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let concise = dexes::perp_concise_annotations(&client).await.unwrap();
    let xyz100 = dexes::perp_annotation(&client, "xyz:XYZ100").await.unwrap().unwrap();
    let btc = dexes::perp_annotation(&client, "BTC").await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"perpConciseAnnotations"}"#,
            r#"{"type":"perpAnnotation","coin":"xyz:XYZ100"}"#,
            r#"{"type":"perpAnnotation","coin":"BTC"}"#,
        ]
    );
    let (_, listed) = concise.iter().find(|(coin, _)| coin == "xyz:XYZ100").unwrap();
    assert_eq!(
        (listed.category.as_str(), listed.display_name.as_deref(), &listed.keywords),
        (xyz100.category.as_str(), xyz100.display_name.as_deref(), &xyz100.keywords),
        "the concise annotation is the whole one without its description"
    );
    assert_eq!(btc, None);
}
