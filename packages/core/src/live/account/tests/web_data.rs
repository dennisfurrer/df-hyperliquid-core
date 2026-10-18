use crate::live::Frame;
use crate::live::fixtures::{FIRST, SECOND, address, dec, mainnet, read};
use crate::utils::Dec;

#[test]
fn open_orders_spot_balances_and_web_data_read_what_they_say() {
    let Frame::OpenOrders(open) = read(&mainnet("openOrders")) else { panic!("orders") };
    assert_eq!((open.dex.as_str(), open.user, open.orders.len()), ("", address(SECOND), 1));
    let order = &open.orders[0];
    assert_eq!(
        (order.limit_px, order.sz, order.orig_sz, order.trigger_px),
        (dec("80459.0"), dec("0.20706"), dec("0.20706"), Dec::ZERO)
    );
    assert_eq!(
        (order.order_type.word(), order.tif.as_deref(), order.trigger_condition.as_str()),
        ("Limit", Some("Alo"), "N/A")
    );
    assert!(!order.is_trigger && !order.is_position_tpsl && !order.reduce_only && order.children.is_empty());
    let Frame::OpenOrders(none) = read(&mainnet("openOrders.empty")) else { panic!("orders") };
    assert!(none.orders.is_empty());

    let Frame::SpotState(spot) = read(&mainnet("spotState")) else { panic!("spot") };
    assert_eq!((spot.user, spot.state.balances.len()), (address(FIRST), 4));
    let usdc = &spot.state.balances[0];
    assert_eq!(
        (usdc.coin.as_str(), usdc.token, usdc.total, usdc.hold),
        ("USDC", Some(0), dec("19042.782306"), dec("9001.9958"))
    );
    assert_eq!(spot.state.token_to_available_after_maintenance, [(0, dec("16840.184972"))]);
    assert_eq!(spot.state.portfolio_margin_enabled, None, "the socket does not say");

    let Frame::WebData3(web) = read(&mainnet("webData3")) else { panic!("web data") };
    let state = &web.user_state;
    assert_eq!((state.user, state.agent_address, state.agent_valid_until), (address(SECOND), None, None));
    assert_eq!((state.cum_ledger, state.server_time, state.is_vault), (dec("-248771.0"), 1_791_480_444_991, false));
    assert_eq!((state.abstraction.as_deref(), state.dex_abstraction_enabled), (Some("disabled"), false));
    assert_eq!(web.perp_dex_states.len(), 11);
    assert_eq!(
        web.perp_dex_states[0].perps_at_open_interest_cap,
        ["CANTO", "FTM", "JELLY", "LOOM", "RLB", "VINE", "ZEREBRO"]
    );
    assert!(web.perp_dex_states[1].perps_at_open_interest_cap.is_empty());

    assert_eq!(
        read(&mainnet("notification")),
        Frame::Notification("Resting order filled: Bought 0.00124 BTC at $80462".to_owned())
    );
}
