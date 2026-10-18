//! `webData3`: the account as Hyperliquid's own frontend reads it, and short notifications.

use crate::live::Frame;
use crate::live::fixtures::{SECOND, address, dec, mainnet, read, vault};

/// The user state and each perp dex's, as the frontend reads them, and a notification's text. A
/// vault that leads vaults of its own names them; an account that leads none is sent no key.
#[test]
fn web_data_and_notifications_read_what_they_say() {
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
    assert!(web.perp_dex_states.iter().all(|dex| dex.leading_vaults.is_empty()));

    let Frame::WebData3(hlp) = read(&vault("webData3")) else { panic!("web data") };
    assert_eq!((hlp.user_state.is_vault, hlp.user_state.abstraction.as_deref()), (true, None));
    assert_eq!(hlp.perp_dex_states[0].total_vault_equity, dec("139705345.523191005"));
    let led: Vec<(String, &str)> =
        hlp.perp_dex_states[0].leading_vaults.iter().map(|led| (led.address.to_hex(), led.name.as_str())).collect();
    assert_eq!(
        (led.len(), led[0].0.as_str(), led[0].1),
        (7, "0x010461c14e146ac35fe42271bdc1134ee31c703a", "HLP Strategy A")
    );
    assert_eq!(hlp.perp_dex_states[8].perps_at_open_interest_cap, ["para:PAPER"]);

    assert_eq!(
        read(&mainnet("notification")),
        Frame::Notification("Resting order filled: Bought 0.00124 BTC at $80462".to_owned())
    );
}
