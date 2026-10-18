//! What each mode lets an order draw on, from one account made by hand: USDC and USDT0 in spot, 10
//! HYPE, and something on the main dex and on `xyz`. The live unified and standard accounts run in
//! the mocked journeys.

use crate::client::USDC_INDEX as USDC;

use super::*;
use crate::account::fixtures::{dec, read};
use crate::account::holdings::perps::{ClearinghouseState, MarginSummary};
use crate::account::holdings::spot::{SpotBalance, SpotState};
use crate::account::holdings::standing::DexState;
use crate::account::settings::mode::Abstraction;

/// USDT0's token index, which backs the `cash` dex.
const USDT: u32 = 268;

/// Each dex's `collateralToken`, as mainnet `allPerpMetas` lists them: USDC for the main dex, `xyz`
/// and `abcd`, USDH (360) for `flx`, USDE (235) for `hyna`, USDT0 for `cash`.
fn collateral() -> Collateral {
    [("", 0), ("xyz", 0), ("abcd", 0), ("flx", 360), ("hyna", 235), ("cash", USDT)]
        .into_iter()
        .map(|(dex, token)| (dex.to_owned(), token))
        .collect()
}

fn dex(name: &str, withdrawable: &str) -> DexState {
    DexState {
        dex: name.to_owned(),
        state: ClearinghouseState {
            margin_summary: MarginSummary { account_value: dec(withdrawable), ..MarginSummary::default() },
            withdrawable: dec(withdrawable),
            ..ClearinghouseState::default()
        },
    }
}

fn row(coin: &str, token: u32, total: &str, hold: &str) -> SpotBalance {
    SpotBalance {
        coin: coin.to_owned(),
        token: Some(token),
        total: dec(total),
        hold: dec(hold),
        ..SpotBalance::default()
    }
}

/// 500 USDC with 120 held, 80 USDT0, 10 HYPE; 100 on the main dex and 25 on `xyz`.
fn standing(mode: Option<Abstraction>) -> Standing {
    Standing {
        mode,
        dexes: vec![dex("", "100"), dex("xyz", "25")],
        spot: Some(SpotState {
            balances: vec![row("USDC", USDC, "500", "120"), row("USDT0", USDT, "80", "0"), row("HYPE", 150, "10", "0")],
            ..SpotState::default()
        }),
        heard: None,
    }
}

fn hype_at_40(coin: &str) -> Option<Dec> {
    match coin {
        "HYPE" => Some(dec("40")),
        "UBTC" => Some(dec("80000")),
        _ => None,
    }
}

#[test]
fn each_mode_draws_each_market_on_the_balance_it_keeps_for_it() {
    use Abstraction::{DexAbstraction, Disabled, UnifiedAccount};
    let perps = |dex: &'static str| Surface::Perps { dex };
    let own = |dex: &str| Source::Perps { dex: dex.to_owned() };
    let spot = Surface::Spot { quote: USDC };
    for (mode, surface, amount, token, from) in [
        (Some(Disabled), perps(""), "100", USDC, own("")),
        (Some(Disabled), perps("xyz"), "25", USDC, own("xyz")),
        (Some(Disabled), spot, "380", USDC, Source::Spot),
        (Some(Disabled), Surface::Outcomes, "380", USDC, Source::Spot),
        (Some(Disabled), Surface::Spot { quote: USDT }, "80", USDT, Source::Spot),
        (Some(Disabled), perps("cash"), "0", USDT, own("cash")),
        (None, perps("xyz"), "25", USDC, own("xyz")),
        (Some(UnifiedAccount), perps("xyz"), "380", USDC, Source::Unified),
        (Some(UnifiedAccount), perps("cash"), "80", USDT, Source::Unified),
        (Some(UnifiedAccount), spot, "380", USDC, Source::Unified),
        (Some(UnifiedAccount), Surface::Outcomes, "380", USDC, Source::Unified),
        (Some(DexAbstraction), perps("xyz"), "100", USDC, Source::NativePerps),
        (Some(DexAbstraction), perps(""), "100", USDC, own("")),
        (Some(DexAbstraction), perps("cash"), "0", USDT, own("cash")),
        (Some(DexAbstraction), spot, "380", USDC, Source::Spot),
    ] {
        let standing = standing(mode.clone());
        let usable = standing.usable(surface, &collateral(), hype_at_40);
        let expected = Usable { amount: dec(amount), token, from, borrowable: Dec::ZERO };
        assert_eq!(usable, expected, "{mode:?} on {surface:?}");
    }
    let mut unread = standing(None);
    assert!(!unread.pools(), "a mode not read is standard");
    if let Some(spot) = unread.spot.as_mut() {
        spot.portfolio_margin_enabled = Some(true);
    }
    assert_eq!((unread.keeping(), unread.pools()), (Keeping::Portfolio, true), "unless spot says portfolio margin");
}

/// A change to the spot answer, made before a case is read.
type Change = fn(&mut SpotState);

/// 10 HYPE at 40 is 400, lent against at 0.65 unless the row says its own; 0.01 BTC at 80000 at
/// 0.5. Less what is owed, held to what USDC has free after maintenance, and nothing once
/// liquidatable. USDT0 borrows nothing.
#[test]
fn portfolio_margin_adds_what_it_would_lend_against_its_collateral() {
    let usdc = Surface::Spot { quote: USDC };
    let cases: [(Change, Surface<'_>, &str, &str); 6] = [
        (|_| {}, Surface::Perps { dex: "" }, "640", "260"),
        (|spot| spot.balances[0].borrowed = Some(dec("60")), usdc, "580", "200"),
        (|_| {}, Surface::Perps { dex: "cash" }, "80", "0"),
        (|spot| spot.balances.push(row("UBTC", 197, "0.01", "0")), usdc, "1040", "660"),
        (
            |spot| {
                spot.balances[2].ltv = Some(dec("0.5"));
                spot.token_to_available_after_maintenance = vec![(USDC, dec("150")), (150, dec("10"))];
            },
            usdc,
            "530",
            "150",
        ),
        (|spot| spot.portfolio_margin_ratio = Some(dec("0.96")), usdc, "380", "0"),
    ];
    for (change, surface, amount, borrowable) in cases {
        let mut standing = standing(Some(Abstraction::PortfolioMargin));
        change(standing.spot.as_mut().unwrap());
        let usable = standing.usable(surface, &collateral(), hype_at_40);
        assert_eq!(
            (usable.amount, usable.borrowable, usable.from),
            (dec(amount), dec(borrowable), Source::PortfolioMargin),
            "{surface:?}: {amount}"
        );
    }
}

/// Portfolio margin, live 2026-10-06: the venue's own room to trade, `total - hold` of USDC
/// (0.55817531 + 268.81821068), is what HYPE lends at 0.65, less the 89.3303277 owed. HYPE/USDC
/// traded between 91.375 and 92.791 in the two hours around the read (`candleSnapshot` of `@107`).
#[test]
fn portfolio_margin_s_room_is_the_venue_s_own_negative_hold() {
    let spot: SpotState = read(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"0.55817531","hold":"-268.81821068","entryNtl":"0.0","spotHold":"0.55817531","ltv":"0.0","borrowed":"89.3303277"},{"coin":"HYPE","token":150,"total":"6.02901107","hold":"1.50143146","entryNtl":"0.0","ltv":"0.65","supplied":"6.02901107"}]}"#,
    );
    let standing = Standing { mode: Some(Abstraction::PortfolioMargin), spot: Some(spot), ..Standing::default() };
    let at = |price: &'static str| {
        standing.usable(Surface::Outcomes, &collateral(), move |coin: &str| (coin == "HYPE").then(|| dec(price)))
    };
    let (low, high) = (at("91.375"), at("92.791"));
    assert_eq!(low.amount, dec("268.7552485388125"), "nothing free, all of it lent");
    assert_eq!(high.amount, dec("274.3043503276405"));
    let venue = dec("0.55817531").checked_add(dec("268.81821068")).unwrap();
    assert!(low.amount <= venue && venue <= high.amount, "{venue} within {} and {}", low.amount, high.amount);
}
