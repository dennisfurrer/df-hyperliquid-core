//! Every vector below was produced by `@nktkas/hyperliquid` 0.33.2, as
//! hip4-tui recorded it: the action put in key order by `canonicalize`
//! against the SDK's own request schema, hashed by `createL1ActionHash` at
//! [`NONCE`] (the connection id), and signed by `signL1Action` with [`KEY`]
//! through viem. Mainnet rows only. The last two are the official Python
//! SDK's own `signing_test.py` vectors, at nonce 0.

use hl_core::{Address, Asset, BuilderFee, Dec, L1Action, LocalKey, digest_of_wallet_json, hex, recover};

use crate::{
    cancel::{Cancel, CancelByCloid},
    grid::Grid,
    leverage::{MarginMode, UpdateIsolatedMargin, UpdateLeverage},
    modify::{BatchModify, Modify, OrderRef},
    order::*,
    twap::{Twap, TwapCancel},
};

/// The key `hyperliquid-python-sdk` signs its tests with.
const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_712_345_678_000;
/// hip4-tui's builder, as its vectors name it.
const TERMINAL: &str = "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843";

fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn asset(id: u32) -> Asset {
    Asset::from_id(id)
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn limit(id: u32, side: Side, price: &str, size: &str, tif: Tif) -> Order {
    Order::limit(asset(id), side, dec(price), dec(size), tif)
}

fn trigger(id: u32, side: Side, price: &str, size: &str, at: &str, tpsl: Tpsl, is_market: bool) -> Order {
    Order::trigger(asset(id), side, dec(price), dec(size), dec(at), tpsl, is_market).reduce_only()
}

fn through(order: Order, builder: &str, fee: u64) -> PlaceOrder {
    PlaceOrder::one(order).through(Builder::new(address(builder), BuilderFee::from_tenths(fee)))
}

/// `(action, posted json, msgpack, connection id, r, s, v)`.
type Vector = (Box<dyn L1Action>, &'static str, &'static str, &'static str, &'static str, &'static str, u8);

fn vectors() -> Vec<Vector> {
    vec![
        // a_perp_market_buy_is_frontend_market_with_its_bound
        (
            Box::new(PlaceOrder::one(Order::market(asset(0), Side::Buy, dec("86501"), dec("0.0012")))),
            r#"{"type":"order","orders":[{"a":0,"b":true,"p":"86501","s":"0.0012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#,
            "83a474797065a56f72646572a66f72646572739186a16100a162c3a170a53836353031a173a6302e30303132a172c2a17481a56c696d697481a3746966ae46726f6e74656e644d61726b6574a867726f7570696e67a26e61",
            "0xda7ac737326c0525928283e95ab1194cf1ad734d19ecd847f9ac048f7f90b9a4",
            "0x63d886cff9e2abc1ba49419844fa8c4b7b4cc60b72424256adaf69a132f2522e",
            "0x3e62642982b99605190a8d59d3766ca03e0ce9bb240afb1cb53372e1f708eaab",
            27,
        ),
        // a_market_buy_through_the_terminal_s_builder_signs_as_the_sdk_does
        (
            Box::new(through(Order::market(asset(0), Side::Buy, dec("86501"), dec("0.0012")), TERMINAL, 0)),
            r#"{"type":"order","orders":[{"a":0,"b":true,"p":"86501","s":"0.0012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":0}}"#,
            "84a474797065a56f72646572a66f72646572739186a16100a162c3a170a53836353031a173a6302e30303132a172c2a17481a56c696d697481a3746966ae46726f6e74656e644d61726b6574a867726f7570696e67a26e61a76275696c64657282a162d92a307836393637643761636431656335663231306539353930363636343938653063633564313361383433a16600",
            "0xcf9b651fad75105f476fc4c35517491458eeab96bd3f04fff16668dc1ea24ac2",
            "0xbd034212c65475345a630798e74f926a7fe382c88ea5b96f5c7b7270cb326751",
            "0x3c215753b1d5be3f91c76717c8c868bba749d4e4085bf47f714e7afcf18dad4d",
            27,
        ),
        // a_hip3_post_only_carries_its_cloid_and_the_builder_last
        (
            Box::new(through(
                limit(110_003, Side::Sell, "340.18", "2.5", Tif::Alo)
                    .reduce_only()
                    .with_cloid(Cloid::from_u128(0xabcd)),
                "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704",
                10,
            )),
            r#"{"type":"order","orders":[{"a":110003,"b":false,"p":"340.18","s":"2.5","r":true,"t":{"limit":{"tif":"Alo"}},"c":"0x0000000000000000000000000000abcd"}],"grouping":"na","builder":{"b":"0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704","f":10}}"#,
            "84a474797065a56f72646572a66f72646572739187a161ce0001adb3a162c2a170a63334302e3138a173a3322e35a172c3a17481a56c696d697481a3746966a3416c6fa163d92230783030303030303030303030303030303030303030303030303030303061626364a867726f7570696e67a26e61a76275696c64657282a162d92a307861623564626330353736323862633138353233633463646663306531653265626462656362373034a1660a",
            "0x3c0292970a3be9a52b59c8dbc88ae6ce471c6025d5d1af83eee1bd93b4ed8fb9",
            "0x6358cd2ef12e4516668b177427b18f0df05b33f946ef6ba5c04bcdd092ffd8d7",
            "0x0316f7267a27ea0bc33304b3aa855e416f083cd8bd09a4995390c76dde75be2c",
            27,
        ),
        // a_spot_order_names_ten_thousand_and_the_pair_index
        (
            Box::new(PlaceOrder::one(limit(10_107, Side::Buy, "44.123", "1.25", Tif::Ioc))),
            r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"44.123","s":"1.25","r":false,"t":{"limit":{"tif":"Ioc"}}}],"grouping":"na"}"#,
            "83a474797065a56f72646572a66f72646572739186a161cd277ba162c3a170a634342e313233a173a4312e3235a172c2a17481a56c696d697481a3746966a3496f63a867726f7570696e67a26e61",
            "0x854910b431cce7b67a4a659984dd8197b608557d5ddfb5a1b91e378643daeb89",
            "0xed3919e1b826e47ecb01a556c6dc3ae3da8b0c9274575e7192013b8efd35f78f",
            "0x7cf3dba925b0570248dc424cc51deac8a59cc0d7a4aec6d16d1a912e6a94effa",
            28,
        ),
        // a_stop_loss_at_market_writes_is_market_then_the_trigger_then_the_kind
        (
            Box::new(PlaceOrder::one(trigger(1, Side::Sell, "2900", "0.5", "3000", Tpsl::StopLoss, true))),
            r#"{"type":"order","orders":[{"a":1,"b":false,"p":"2900","s":"0.5","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"3000","tpsl":"sl"}}}],"grouping":"na"}"#,
            "83a474797065a56f72646572a66f72646572739186a16101a162c2a170a432393030a173a3302e35a172c3a17481a77472696767657283a869734d61726b6574c3a9747269676765725078a433303030a47470736ca2736ca867726f7570696e67a26e61",
            "0x3ef73d1a5b16b1adeae1d3e680a7d9365050a42251073653388943b0f3f1f250",
            "0x2d36fcce99d8004fab1890e6a07cc39e19aa38306191ba8734b4a6819ad89e6c",
            "0x5683161e5ae9c539d2dad38e1dfe4ce64d0b48520c775319dfaac4750203f364",
            28,
        ),
        // a_take_profit_at_a_limit
        (
            Box::new(PlaceOrder::one(trigger(1, Side::Sell, "3500", "0.5", "3490", Tpsl::TakeProfit, false))),
            r#"{"type":"order","orders":[{"a":1,"b":false,"p":"3500","s":"0.5","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"3490","tpsl":"tp"}}}],"grouping":"na"}"#,
            "83a474797065a56f72646572a66f72646572739186a16101a162c2a170a433353030a173a3302e35a172c3a17481a77472696767657283a869734d61726b6574c2a9747269676765725078a433343930a47470736ca27470a867726f7570696e67a26e61",
            "0x29512775bcb00cd64cd3cbf3640fc060f33d609db9fce207073698410871b3fa",
            "0x3118a99e7d5d8b97a0d9cd1bca1996568cb2a849f13c27b5dc61f45308d9df4f",
            "0x198ab28bc0808c9a2028ad75730e2bfa0d8498df760e40cae471119c32a3147a",
            27,
        ),
        // a_scale_is_one_action_with_a_leg_per_level
        (
            Box::new(PlaceOrder::new(vec![
                limit(5, Side::Buy, "150", "1", Tif::Gtc),
                limit(5, Side::Buy, "147.5", "1", Tif::Gtc),
                limit(5, Side::Buy, "145", "1", Tif::Gtc),
            ])),
            r#"{"type":"order","orders":[{"a":5,"b":true,"p":"150","s":"1","r":false,"t":{"limit":{"tif":"Gtc"}}},{"a":5,"b":true,"p":"147.5","s":"1","r":false,"t":{"limit":{"tif":"Gtc"}}},{"a":5,"b":true,"p":"145","s":"1","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#,
            "83a474797065a56f72646572a66f72646572739386a16105a162c3a170a3313530a173a131a172c2a17481a56c696d697481a3746966a347746386a16105a162c3a170a53134372e35a173a131a172c2a17481a56c696d697481a3746966a347746386a16105a162c3a170a3313435a173a131a172c2a17481a56c696d697481a3746966a3477463a867726f7570696e67a26e61",
            "0xe11a75e4e73699e670466c4df26698c8d2dc2ad1df066978b712303d75fa354e",
            "0xeda726e32bdd7cdfa669e7624aaf15f6edbabbee2e6adc5850af870a58132824",
            "0x56ef5487dd87939727a4c94cc53b23082c7ab948132f0bd4d54075237f4067bd",
            27,
        ),
        // a_cancel_names_asset_and_order
        (
            Box::new(Cancel::new(vec![(asset(0), 123_456_789), (asset(110_003), 42)])),
            r#"{"type":"cancel","cancels":[{"a":0,"o":123456789},{"a":110003,"o":42}]}"#,
            "82a474797065a663616e63656ca763616e63656c739282a16100a16fce075bcd1582a161ce0001adb3a16f2a",
            "0xc11374adc94b441cd55acef00faff48ed25f6cffb3656cac16dd90bd3747ad20",
            "0xffb5d563b42e91ecdbba926240f40bdd33ebe62d5b9fbdf2c424c0ae48d5167d",
            "0x751e0ffc53b401997ee5803221750d9c1bdc3b85ea51e912fe8cc0c26e8d0499",
            28,
        ),
        // a_batch_modify_puts_the_order_id_before_the_new_order
        (
            Box::new(BatchModify::new(vec![Modify::new(
                OrderRef::Oid(987_654_321),
                limit(0, Side::Buy, "84000", "0.01", Tif::Gtc),
            )])),
            r#"{"type":"batchModify","modifies":[{"oid":987654321,"order":{"a":0,"b":true,"p":"84000","s":"0.01","r":false,"t":{"limit":{"tif":"Gtc"}}}}]}"#,
            "82a474797065ab62617463684d6f64696679a86d6f6469666965739182a36f6964ce3ade68b1a56f7264657286a16100a162c3a170a53834303030a173a4302e3031a172c2a17481a56c696d697481a3746966a3477463",
            "0xfe90a087e179a1ea3ae567cf84dc873f9eeb9f0f94c21b3bc5f461e78d532da7",
            "0x022d104cec25ea49408e6a939473edbcd4e5b6eb87fc6ba6bb20da02a5defbe9",
            "0x73d3aeb69cb7026e6a33ba53ebef036ceb3299b528ab7be42b5cce45ba76cd6a",
            27,
        ),
        // a_batch_modify_of_a_stop_at_market_ends_with_always_place
        (
            Box::new(BatchModify::new(vec![Modify::new(
                OrderRef::Oid(987_654_321),
                trigger(0, Side::Sell, "79200", "0.001", "80000", Tpsl::StopLoss, true),
            )])),
            r#"{"type":"batchModify","modifies":[{"oid":987654321,"order":{"a":0,"b":false,"p":"79200","s":"0.001","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"80000","tpsl":"sl"}}}}],"a":true}"#,
            "83a474797065ab62617463684d6f64696679a86d6f6469666965739182a36f6964ce3ade68b1a56f7264657286a16100a162c2a170a53739323030a173a5302e303031a172c3a17481a77472696767657283a869734d61726b6574c3a9747269676765725078a53830303030a47470736ca2736ca161c3",
            "0x37641361c2934f3188450336d1fbae7b381f559f24d302734390f7e1a56513c8",
            "0x186fa03a789b578d724d700c39f70e9687bf41a443e87a709c02958732c5a376",
            "0x4bef49f1973b5e6c84265b761394f3a938dfcd0fc2bb94e7e3357e0b2a8b3144",
            27,
        ),
        // a_batch_modify_of_a_hip3_take_profit_at_a_limit
        (
            Box::new(BatchModify::new(vec![Modify::new(
                OrderRef::Oid(42),
                trigger(110_003, Side::Buy, "380.5", "2.5", "381", Tpsl::TakeProfit, false),
            )])),
            r#"{"type":"batchModify","modifies":[{"oid":42,"order":{"a":110003,"b":true,"p":"380.5","s":"2.5","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"381","tpsl":"tp"}}}}],"a":true}"#,
            "83a474797065ab62617463684d6f64696679a86d6f6469666965739182a36f69642aa56f7264657286a161ce0001adb3a162c3a170a53338302e35a173a3322e35a172c3a17481a77472696767657283a869734d61726b6574c2a9747269676765725078a3333831a47470736ca27470a161c3",
            "0x253563913feb93d07a95ce12f1fd5c26080ee67055bf618b2cd4cd2d3fad8f6c",
            "0xefde93a6171f6d75e022ea3372257fbfc3a801b840c61d41a0956916e30d1732",
            "0x7984e9be2f62472275142e86775c35f569c17024ca148276c6e2b12d1ccc6ec3",
            27,
        ),
        // a_batch_modify_of_a_position_stop_keeps_size_zero
        (
            Box::new(BatchModify::new(vec![Modify::new(
                OrderRef::Oid(7),
                trigger(1, Side::Sell, "2900", "0", "3000", Tpsl::StopLoss, true),
            )])),
            r#"{"type":"batchModify","modifies":[{"oid":7,"order":{"a":1,"b":false,"p":"2900","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"3000","tpsl":"sl"}}}}],"a":true}"#,
            "83a474797065ab62617463684d6f64696679a86d6f6469666965739182a36f696407a56f7264657286a16101a162c2a170a432393030a173a130a172c3a17481a77472696767657283a869734d61726b6574c3a9747269676765725078a433303030a47470736ca2736ca161c3",
            "0x16d7639ff3cf1c5cb2e6ca0c0f80d7dcdca8cf5e9a0b3139a2b2ac08b62b3d8c",
            "0xb61f4e2e717633ce5ef19dff57672ded9a6e8ec2f844aaca0193f051a4fdc96d",
            "0x15fb14fd7110241471abe8f36898ce758a74b8dde64be8ea40df166fa6287b4c",
            28,
        ),
        // a_twap_nests_its_fields_under_twap
        (
            Box::new(Twap::new(asset(0), Side::Buy, dec("0.5"), 30).unwrap().randomized()),
            r#"{"type":"twapOrder","twap":{"a":0,"b":true,"s":"0.5","r":false,"m":30,"t":true}}"#,
            "82a474797065a9747761704f72646572a47477617086a16100a162c3a173a3302e35a172c2a16d1ea174c3",
            "0xd09b419a1e1f39a66afb2ce4d0429a6a9df5b0a1d438dc1f94993f0098f9eeb3",
            "0xecee008a4de938534619bd4561d8d04b4590deeeaabb5249590e741615f11bda",
            "0x5ee3570246d3b2e4143fb2d5a2319d378630bb1780b2d55e00e7dccc947384c7",
            27,
        ),
        // a_twap_cancel_is_flat
        (
            Box::new(TwapCancel::new(asset(0), 1234)),
            r#"{"type":"twapCancel","a":0,"t":1234}"#,
            "83a474797065aa7477617043616e63656ca16100a174cd04d2",
            "0x53decf7c26ace255c774c7f927c75789578271c12af69819bb9cefc3d893591b",
            "0xd5625f79e36dbff982d6621db8b77ff722cde68a616f96ff69ea522165d98c59",
            "0x2c89672738a5ad0ab071cfcd4174544ceb6839718399651b537271260af0dee4",
            28,
        ),
        // leverage_is_asset_then_cross_then_the_figure
        (
            Box::new(UpdateLeverage::new(asset(110_003), MarginMode::Isolated, 10).unwrap()),
            r#"{"type":"updateLeverage","asset":110003,"isCross":false,"leverage":10}"#,
            "84a474797065ae7570646174654c65766572616765a56173736574ce0001adb3a7697343726f7373c2a86c657665726167650a",
            "0x05fa0ed2d44e19efcf1cd92bfaf906ce916b7acf32c858e6843c6f2815f6a771",
            "0xf4e6fa8044c40283512d36c782092b41be5874767619e398e2072a10c11498bb",
            "0x3b518a99d56c43ee6ba4e6f14b6a70a4df9eb072fb37ac27efcc8c60c444c188",
            28,
        ),
        // margin_goes_in_as_millionths
        (
            Box::new(UpdateIsolatedMargin::new(asset(1), Side::Buy, dec("12.5")).unwrap()),
            r#"{"type":"updateIsolatedMargin","asset":1,"isBuy":true,"ntli":12500000}"#,
            "84a474797065b475706461746549736f6c617465644d617267696ea5617373657401a56973427579c3a46e746c69ce00bebc20",
            "0xa654cae263683a26e92d2d5895db24b49960d980af06c2a73243391218fe8636",
            "0x8991416a4c2fbdfa9ef2a954a255657052c8d37b3c975fe34fb4959c780ea607",
            "0x789236e106c2acf5edd956053ffafed670aaacb175b13bc85d0b5046927cde6b",
            28,
        ),
        // margin_comes_out_as_a_negative_int32
        (
            Box::new(UpdateIsolatedMargin::new(asset(1), Side::Buy, dec("-2.5")).unwrap()),
            r#"{"type":"updateIsolatedMargin","asset":1,"isBuy":true,"ntli":-2500000}"#,
            "84a474797065b475706461746549736f6c617465644d617267696ea5617373657401a56973427579c3a46e746c69d2ffd9da60",
            "0x4fe5cc74be136b09542a3c903c9ffba4f542bc484cc6b32f5bcd78d1d17d79a0",
            "0xba58a0f9ea7965a8962a900d2bdb81bd167ac178149a8d6e0b8b305aafa55ee8",
            "0x376f3d414ca618d502cec67d41371bba66f59f9d2228a6462258203e4b35a4b9",
            28,
        ),
        // an_order_at_each_market_s_fee_signs_as_the_sdk_does: a perp at 88
        (
            Box::new(through(Order::market(asset(0), Side::Buy, dec("86501"), dec("0.0012")), TERMINAL, 88)),
            r#"{"type":"order","orders":[{"a":0,"b":true,"p":"86501","s":"0.0012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":88}}"#,
            "84a474797065a56f72646572a66f72646572739186a16100a162c3a170a53836353031a173a6302e30303132a172c2a17481a56c696d697481a3746966ae46726f6e74656e644d61726b6574a867726f7570696e67a26e61a76275696c64657282a162d92a307836393637643761636431656335663231306539353930363636343938653063633564313361383433a16658",
            "0xd3279f475d9f4d267bf239d58ba770327b5b43b1412869d56a855e411f769f9f",
            "0x0e55d78251abd0ce6d3b4a2bd32bb5ba30fbb41f296aa14a3ef70ab4cdb069c2",
            "0x38f4fb141ab39c07334ae1c60c6041b8fa667d2ffc7bc9e836fa578747e8d61a",
            27,
        ),
        // a HIP-3 perp at 88
        (
            Box::new(through(limit(110_001, Side::Sell, "340.18", "2.5", Tif::Gtc), TERMINAL, 88)),
            r#"{"type":"order","orders":[{"a":110001,"b":false,"p":"340.18","s":"2.5","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":88}}"#,
            "84a474797065a56f72646572a66f72646572739186a161ce0001adb1a162c2a170a63334302e3138a173a3322e35a172c2a17481a56c696d697481a3746966a3477463a867726f7570696e67a26e61a76275696c64657282a162d92a307836393637643761636431656335663231306539353930363636343938653063633564313361383433a16658",
            "0xded8dfb64c97e501dab6d21ab32d985d14ad05177f3b3e1bf6a3a1028439ab68",
            "0x0997aed0b3469b2a19cf4e3aaca9492f1e85e8319dfc675f6c2cc7061aeb3f8c",
            "0x4414097fa4dc084a5c8ecb8e86a9dbbbbc4e2e03bae80435503607ed6216eaa3",
            28,
        ),
        // a spot pair at 880
        (
            Box::new(through(limit(10_107, Side::Buy, "44", "1", Tif::Gtc), TERMINAL, 880)),
            r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"44","s":"1","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":880}}"#,
            "84a474797065a56f72646572a66f72646572739186a161cd277ba162c3a170a23434a173a131a172c2a17481a56c696d697481a3746966a3477463a867726f7570696e67a26e61a76275696c64657282a162d92a307836393637643761636431656335663231306539353930363636343938653063633564313361383433a166cd0370",
            "0xea5429ccf191d81c3d9b54df28da1706a0d66fbed3fc4cb69507f81dd603251b",
            "0x593bff843658f415fe04aafd70545b61c8787e30afcfdd2546fbe02b6241a559",
            "0x4cc5afbfe92fffa4cc2131eaedff9ac1cbf0600eaa1a922f4255dabf07fbbbcb",
            28,
        ),
        // a HIP-4 outcome at 880, a market buy at the edge of the range
        (
            Box::new(through(Order::market(asset(100_012_100), Side::Buy, dec("0.99999"), dec("20")), TERMINAL, 880)),
            r#"{"type":"order","orders":[{"a":100012100,"b":true,"p":"0.99999","s":"20","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":880}}"#,
            "84a474797065a56f72646572a66f72646572739186a161ce05f61044a162c3a170a7302e3939393939a173a23230a172c2a17481a56c696d697481a3746966ae46726f6e74656e644d61726b6574a867726f7570696e67a26e61a76275696c64657282a162d92a307836393637643761636431656335663231306539353930363636343938653063633564313361383433a166cd0370",
            "0xaa33e9f4b217c773b1b2497f6eef60caf9bba607f1ec2d61760fd0362c4c1418",
            "0x7d8e36e1fd2cb1430bce6a6fdebb1ae2d90fbbdb0f362ac66e6e0035b48cdc97",
            "0x3ccfaceaaf6b3ec5358b5bc861f46164ce799cf30c65c7ded742a33b34bfe953",
            28,
        ),
    ]
}

#[test]
fn every_trading_action_is_the_sdk_s_byte_for_byte() {
    let key = key();
    for (action, json, msgpack, id, r, s, v) in vectors() {
        assert_eq!(action.wire(), json, "the posted action");
        assert_eq!(hex::encode(&action.msgpack()), msgpack, "{json}: the msgpack");
        assert_eq!(hex::encode_prefixed(&action.connection_id(NONCE, None)), id, "{json}: the connection id");
        let digest = action.digest(NONCE, None);
        let signature = key.sign_digest(&digest).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(recover(&digest, &signature), Ok(key.address()));
        assert_eq!(digest_of_wallet_json(&action.wallet_json(NONCE, None)), Ok(digest), "{json}: wallet json");
        assert_eq!(
            action.body(NONCE, &signature, None),
            format!(
                r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}},"vaultAddress":null}}"#
            ),
            "{json}: the body"
        );
    }
}

/// `test_l1_action_signing_order_matches` and
/// `test_l1_action_signing_tpsl_order_matches`: ETH, asset 1, 100 at 100,
/// nonce 0. Python drops a leading zero from `r` and `s`; these have none.
#[test]
fn the_python_sdk_s_order_vectors_sign_the_same() {
    let key = key();
    let plain = PlaceOrder::one(limit(1, Side::Buy, "100", "100", Tif::Gtc));
    let stop =
        PlaceOrder::one(Order::trigger(asset(1), Side::Buy, dec("100"), dec("100"), dec("103"), Tpsl::StopLoss, true));
    for (action, r, s, v) in [
        (
            plain,
            "0xd65369825a9df5d80099e513cce430311d7d26ddf477f5b3a33d2806b100d78e",
            "0x2b54116ff64054968aa237c20ca9ff68000f977c93289157748a3162b6ea940e",
            28,
        ),
        (
            stop,
            "0x98343f2b5ae8e26bb2587daad3863bc70d8792b09af1841b6fdd530a2065a3f9",
            "0x6b5bb6bb0633b710aa22b721dd9dee6d083646a5f8e581a20b545be6c1feb405",
            27,
        ),
    ] {
        let signature = key.sign_digest(&action.digest(0, None)).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{action:?}");
    }
}

/// The rule the whole family keeps: a market order is `FrontendMarket`, on
/// either side, from the plain constructor and from the grid's, and nothing
/// a market order posts says `Gtc`.
#[test]
fn a_market_order_is_frontend_market_and_never_gtc() {
    for side in [Side::Buy, Side::Sell] {
        let order = Order::market(asset(0), side, dec("86501"), dec("0.0012"));
        assert_eq!(order.tif(), Some(Tif::FrontendMarket), "{side:?}");
        assert!(order.is_market());
        let posted = PlaceOrder::one(order).wire();
        assert!(posted.contains(r#""tif":"FrontendMarket""#), "{posted}");
        assert!(!posted.contains("Gtc"), "{posted}");

        let gridded = Grid::perp(5).market(asset(0), side, dec("86500"), dec("0.01"), dec("0.0012")).unwrap();
        assert_eq!(gridded.tif(), Some(Tif::FrontendMarket), "{side:?}");
        assert!(gridded.is_market());
    }
    let resting = Order::limit(asset(0), Side::Buy, dec("86501"), dec("0.0012"), Tif::Gtc);
    assert!(!resting.is_market(), "a Gtc is a resting limit, not a market order");
    let stop = Order::trigger(asset(0), Side::Sell, dec("1"), dec("1"), dec("2"), Tpsl::StopLoss, true);
    assert_eq!((stop.tif(), stop.is_market(), stop.is_trigger()), (None, false, true));
}

#[test]
fn the_key_order_is_the_wire_order() {
    let order = limit(516, Side::Buy, "0.35", "50", Tif::Gtc);
    assert_eq!(order.wire().keys().collect::<Vec<_>>(), ["a", "b", "p", "s", "r", "t"]);
    assert_eq!(
        order.with_cloid(Cloid::from_u128(1)).wire().keys().collect::<Vec<_>>(),
        ["a", "b", "p", "s", "r", "t", "c"]
    );
    assert_eq!(PlaceOrder::one(order).action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping"]);
    let routed = through(order, TERMINAL, 10);
    assert_eq!(routed.action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping", "builder"]);
    assert_eq!(order.wire().to_msgpack()[0], 0x86, "an absent cloid leaves no key, not a nil");
}

/// hip4-tui's `the_remaining_actions_keep_their_own_spellings`: no SDK
/// vector covers these, so their spellings and key order are pinned here.
#[test]
fn cancel_by_cloid_and_modify_keep_their_own_spellings() {
    let cloid = Cloid::from_u128(9);
    let by_cloid = CancelByCloid::one(asset(100_005_160), cloid);
    assert_eq!(
        by_cloid.wire(),
        r#"{"type":"cancelByCloid","cancels":[{"asset":100005160,"cloid":"0x00000000000000000000000000000009"}]}"#
    );
    let modify = Modify::new(OrderRef::Oid(42), limit(516, Side::Sell, "0.66", "1", Tif::Alo));
    assert_eq!(modify.action().keys().collect::<Vec<_>>(), ["type", "oid", "order"]);
    assert_eq!(modify.entry().keys().collect::<Vec<_>>(), ["oid", "order"]);
    assert!(modify.wire().starts_with(r#"{"type":"modify","oid":42,"order":{"a":516,"#));
    let by_client = Modify::new(OrderRef::Cloid(cloid), limit(516, Side::Sell, "0.66", "1", Tif::Alo));
    assert!(by_client.wire().contains(r#""oid":"0x00000000000000000000000000000009""#));
    assert!(!BatchModify::new(vec![modify]).is_always_place(), "a limit order needs no always place");
    assert!(BatchModify::new(vec![modify]).always_place().wire().ends_with(r#""a":true}"#));
}

#[test]
fn a_cloid_is_thirty_two_hex_digits() {
    let cloid: Cloid = "0x0000000000000000000000000000abcd".parse().unwrap();
    assert_eq!(cloid, Cloid::from_u128(0xabcd));
    assert_eq!(cloid.to_string(), "0x0000000000000000000000000000abcd");
    assert!("0xabcd".parse::<Cloid>().is_err());
}

/// hip4-tui's caps: 0.1% on a perp, main dex or HIP-3, 1% on spot and on an
/// outcome; one perp order holds the action to the perp cap.
#[test]
fn a_builder_takes_at_most_a_tenth_of_a_percent_on_perps_and_one_percent_elsewhere() {
    let paying = |orders: Vec<Order>, fee: u64| {
        PlaceOrder::new(orders)
            .through(Builder::new(address("0x1111111111111111111111111111111111111111"), BuilderFee::from_tenths(fee)))
    };
    let perp = limit(0, Side::Buy, "100", "1", Tif::Gtc);
    let hip3 = limit(110_001, Side::Buy, "100", "1", Tif::Gtc);
    let spot = limit(10_107, Side::Buy, "44", "1", Tif::Gtc);
    let outcome = limit(100_012_100, Side::Buy, "0.5", "20", Tif::Gtc);
    let refused = |action: PlaceOrder| action.over_builder_cap().map(|over| over.to_string());
    assert_eq!(refused(paying(vec![perp], 100)), None);
    assert_eq!(
        refused(paying(vec![perp], 101)).as_deref(),
        Some("the builder fee of 101 is over the 100 a perp order allows (0.1%)")
    );
    assert_eq!(
        refused(paying(vec![hip3], 101)).as_deref(),
        Some("the builder fee of 101 is over the 100 a HIP-3 perp order allows (0.1%)")
    );
    assert_eq!(refused(paying(vec![spot], 1000)), None);
    assert_eq!(
        refused(paying(vec![spot], 1001)).as_deref(),
        Some("the builder fee of 1001 is over the 1000 a spot order allows (1%)")
    );
    assert_eq!(refused(paying(vec![outcome], 1000)), None, "an outcome takes the spot cap");
    assert_eq!(
        refused(paying(vec![outcome], 1001)).as_deref(),
        Some("the builder fee of 1001 is over the 1000 an outcome order allows (1%)")
    );
    assert!(refused(paying(vec![spot, perp], 500)).is_some(), "one perp order holds the action to the perp cap");
    assert_eq!(PlaceOrder::one(perp).over_builder_cap(), None, "no builder, no cap");
}

#[test]
fn what_would_be_refused_is_refused_before_signing() {
    use crate::error::TradeError;

    assert_eq!(Twap::new(asset(0), Side::Buy, dec("1"), 4), Err(TradeError::TwapMinutes(4)));
    assert_eq!(Twap::new(asset(0), Side::Buy, dec("1"), 1441), Err(TradeError::TwapMinutes(1441)));
    assert!(
        Twap::new(asset(0), Side::Buy, dec("1"), 5).is_ok() && Twap::new(asset(0), Side::Buy, dec("1"), 1440).is_ok()
    );
    assert_eq!(UpdateLeverage::new(asset(0), MarginMode::Cross, 0), Err(TradeError::NoLeverage));
    assert!(UpdateLeverage::new(asset(0), MarginMode::Cross, 1).unwrap().wire().contains(r#""isCross":true"#));
    assert!(matches!(
        UpdateIsolatedMargin::new(asset(1), Side::Buy, dec("1.0000001")),
        Err(TradeError::Margin(hl_core::BadUnits::TooFine { places: 6, .. }))
    ));
    assert!(matches!(UpdateIsolatedMargin::new(asset(1), Side::Sell, Dec::ZERO), Err(TradeError::Margin(_))));
    assert_eq!(UpdateIsolatedMargin::new(asset(1), Side::Sell, dec("-0.000001")).unwrap().ntli(), -1);
}
