use crate::account::history::{Delta, FillLiquidation};
use crate::live::Frame;
use crate::live::fixtures::{FIRST, SECOND, address, dec, mainnet, read, rebate};
use crate::markets::Side;
use crate::utils::Dec;

#[test]
fn fills_say_whether_they_are_the_snapshot_and_keep_every_field() {
    let Frame::UserFills(snapshot) = read(&mainnet("userFills.snapshot")) else { panic!("fills") };
    assert!(snapshot.is_snapshot, "the first frame after subscribing replays history");
    assert_eq!((snapshot.user, snapshot.fills.len()), (address(FIRST), 30));
    let first = &snapshot.fills[0];
    assert_eq!((first.px, first.sz, first.side, first.crossed), (dec("80395.0"), dec("0.00014"), Side::Buy, true));
    assert_eq!((first.start_position, first.dir.as_str(), first.fee), (dec("0.66492"), "Open Long", dec("0.005064")));
    assert_eq!(first.cloid.as_deref(), Some("0x000000000000000000065d578980bfcb"));

    let Frame::UserFills(stream) = read(&mainnet("userFills")) else { panic!("fills") };
    assert!(!stream.is_snapshot, "a frame without isSnapshot is new fills");
    assert_eq!((stream.user, stream.fills.as_slice()), (address(SECOND), [rebate()].as_slice()));

    let Frame::UserFills(liquidated) = read(&mainnet("userFills.liquidation")) else { panic!("fills") };
    let fill = &liquidated.fills[18];
    assert_eq!(
        fill.liquidation,
        Some(FillLiquidation {
            liquidated_user: Some(address("0xf467f1f46cad3f1041c765765be2891245144f6e")),
            mark_px: dec("80429.0"),
            method: "market".to_owned(),
        })
    );
    assert_eq!((fill.dir.as_str(), fill.closed_pnl), ("Close Short", dec("16.680024")));
    // One market order closing into three liquidations at the same mark.
    let users: Vec<_> = liquidated
        .fills
        .iter()
        .filter_map(|fill| fill.liquidation.as_ref())
        .map(|liquidation| (liquidation.liquidated_user, liquidation.mark_px))
        .collect();
    assert_eq!(
        users,
        [
            (Some(address("0xf467f1f46cad3f1041c765765be2891245144f6e")), dec("80429.0")),
            (Some(address("0xf65ae6061b0ecbddafb3e04381e4fd8c2cd2f0fd")), dec("80429.0")),
            (Some(address("0xfc472f2c3e4227c332ae3b15d796970b4c1afce4")), dec("80429.0")),
        ]
    );
}

#[test]
fn fundings_and_ledger_updates_keep_their_amounts_exact() {
    let Frame::UserFundings(fundings) = read(&mainnet("userFundings")) else { panic!("fundings") };
    assert_eq!((fundings.user, fundings.is_snapshot, fundings.fundings.len()), (address(FIRST), true, 100));
    let first = &fundings.fundings[0];
    assert_eq!(
        (first.time, first.coin.as_str(), first.usdc, first.szi),
        (1_791_421_200_128, "TAO", dec("-0.025182"), dec("6.917"))
    );
    assert_eq!((first.funding_rate, first.n_samples), (dec("0.0000125"), None));
    let last = &fundings.fundings[99];
    assert_eq!(
        (last.coin.as_str(), last.usdc, last.funding_rate),
        ("xyz:BRENTOIL", dec("-8.101769"), dec("-0.0003825775"))
    );

    let Frame::Ledger(ledger) = read(&mainnet("userNonFundingLedgerUpdates")) else { panic!("ledger") };
    assert_eq!((ledger.user, ledger.is_snapshot, ledger.updates.len()), (address(FIRST), true, 23));
    let deposit = &ledger.updates[0];
    assert_eq!(
        (deposit.time, deposit.kind(), &deposit.delta),
        (1_788_831_761_730, "deposit", &Delta::Deposit { usdc: dec("7.0") })
    );
    assert_eq!(deposit.hash, "0x885308289db123a4efc1afb1aa1070955311838659bd00672be3005d450b7fb4");
    let send = &ledger.updates[1];
    let Delta::Send { amount, fee, destination_dex, nonce, .. } = &send.delta else { panic!("a send") };
    assert_eq!((send.kind(), *amount, *fee), ("send", dec("9.75306"), Dec::ZERO));
    assert_eq!((destination_dex.as_str(), *nonce), ("", Some(3_217_031)));
}
