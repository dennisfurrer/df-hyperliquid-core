//! HLP's leader's ledger, read by window as mainnet answers it: the day he made HLP, his early
//! liquidations, and nothing since his last row.

use df_hyperliquid_core::account::history::{Delta, LedgerUpdate, ledger};
use df_hyperliquid_core::{Client, Dec, Network};

use super::{LEADER, address, bodies, dec};
use crate::support::venue::Venue;
use crate::support::{clock, fixture};

const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
/// The night HLP was made: 2023-05-04 23:30 to 05-05 01:43 UTC.
const MADE: (u64, u64) = (1_683_243_000_000, 1_683_251_000_000);
/// 2023-02-28 21:00 to 03-03 02:20 UTC.
const LIQUIDATED: (u64, u64) = (1_677_618_000_000, 1_677_810_000_000);
/// Just past his last row, 2026-08-16 00:06 UTC.
const SINCE: u64 = 1_786_838_806_142;

fn read(start: u64, rest: &str) -> String {
    format!(r#"{{"type":"userNonFundingLedgerUpdates","user":"{LEADER}","startTime":{start}{rest}}}"#)
}

fn kinds(rows: &[LedgerUpdate]) -> Vec<&str> {
    rows.iter().map(LedgerUpdate::kind).collect()
}

#[tokio::test]
async fn the_ledger_is_read_window_by_window_liquidations_and_all() {
    let window = |start: u64| ("startTime", start.to_string());
    let (made, liquidated, since) = (window(MADE.0), window(LIQUIDATED.0), window(SINCE));
    let venue = Venue::mainnet()
        .info_where(
            "userNonFundingLedgerUpdates",
            &[(made.0, &made.1)],
            &fixture("account/ledger_hlp_made_mainnet.json"),
        )
        .info_where(
            "userNonFundingLedgerUpdates",
            &[(liquidated.0, &liquidated.1)],
            &fixture("account/ledger_liquidated_mainnet.json"),
        )
        .info_where(
            "userNonFundingLedgerUpdates",
            &[(since.0, &since.1)],
            &fixture("account/ledger_since_mainnet.json"),
        );
    let client = Client::reader(venue, Network::Mainnet, clock);
    let leader = address(LEADER);

    let made = ledger(&client, leader, MADE.0, Some(MADE.1)).await.unwrap();
    let liquidated = ledger(&client, leader, LIQUIDATED.0, Some(LIQUIDATED.1)).await.unwrap();
    let since = ledger(&client, leader, SINCE, None).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            read(MADE.0, &format!(r#","endTime":{}"#, MADE.1)),
            read(LIQUIDATED.0, &format!(r#","endTime":{}"#, LIQUIDATED.1)),
            read(SINCE, ""),
        ]
    );
    assert_eq!(
        kinds(&made),
        [
            "vaultCreate",
            "internalTransfer",
            "internalTransfer",
            "vaultDeposit",
            "internalTransfer",
            "vaultDeposit",
            "internalTransfer",
            "vaultDeposit"
        ]
    );
    assert_eq!(made[0].delta, Delta::VaultCreate { vault: address(HLP), usdc: dec("200"), fee: Dec::ZERO });
    let (mut sent_in, mut deposited) = (Dec::ZERO, Dec::ZERO);
    for row in &made {
        match &row.delta {
            Delta::InternalTransfer { usdc, destination, .. } if *destination == leader => {
                sent_in = sent_in.checked_add(*usdc).unwrap();
            }
            Delta::VaultDeposit { vault, usdc } if *vault == address(HLP) => {
                deposited = deposited.checked_add(*usdc).unwrap();
            }
            Delta::VaultCreate { .. } => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!((sent_in, deposited), (dec("37945"), dec("37992.216076")), "what three accounts sent in, into HLP");

    assert_eq!(kinds(&liquidated), ["liquidation", "deposit", "liquidation"]);
    let Delta::Liquidation { liquidated_ntl_pos, account_value, leverage_type, liquidated_positions } =
        &liquidated[0].delta
    else {
        panic!("not a liquidation: {:?}", liquidated[0].delta);
    };
    assert_eq!(
        (*liquidated_ntl_pos, *account_value, leverage_type.as_str()),
        (dec("356.80015"), dec("2.956494"), "Cross")
    );
    let positions: Vec<(&str, Dec)> =
        liquidated_positions.iter().map(|position| (position.coin.as_str(), position.szi)).collect();
    assert_eq!(positions, [("ATOM", dec("29.21"))]);
    assert!(since.is_empty());
}
