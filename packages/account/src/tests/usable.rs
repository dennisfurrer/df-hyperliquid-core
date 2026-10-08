use super::*;
use crate::{
    fixtures::{
        PERP_STANDARD, PERP_STANDARD_XYZ, PERP_UNIFIED, PERP_UNIFIED_XYZ, SPOT_STANDARD, SPOT_UNIFIED, dec, read,
    },
    state::{MarginSummary, SpotBalance},
};

/// USDT0's token index, which backs the `cash` dex.
const USDT: u32 = 268;

/// Each dex's `collateralToken`, as hip4-tui's recorded mainnet
/// `allPerpMetas` lists them: USDC for the main dex, `xyz` and `abcd`, USDH
/// (360) for `flx`, USDE (235) for `hyna`, USDT0 for `cash`.
fn collateral() -> Collateral {
    [("", 0), ("xyz", 0), ("abcd", 0), ("flx", 360), ("hyna", 235), ("cash", USDT)]
        .into_iter()
        .map(|(dex, token)| (dex.to_owned(), token))
        .collect()
}

fn dex(name: &str, withdrawable: &str) -> DexState {
    DexState {
        dex: name.to_owned(),
        state: PerpState {
            margin_summary: MarginSummary { account_value: dec(withdrawable), ..MarginSummary::default() },
            withdrawable: dec(withdrawable),
            ..PerpState::default()
        },
    }
}

fn row(coin: &str, token: u32, total: &str, hold: &str, borrowed: &str) -> SpotBalance {
    SpotBalance {
        coin: coin.to_owned(),
        token: Some(token),
        total: dec(total),
        hold: dec(hold),
        borrowed: Some(dec(borrowed)).filter(|owed| !owed.is_zero()),
        ..SpotBalance::default()
    }
}

/// hip4-tui's `engine/src/tests/accounts.rs`: USDC and USDT in spot, 10
/// HYPE, and something on the main dex and on `xyz`.
fn standing(mode: Abstraction) -> Standing {
    Standing {
        mode: Some(mode),
        dexes: vec![dex("", "100"), dex("xyz", "25")],
        spot: Some(SpotState {
            balances: vec![
                row("USDC", USDC, "500", "120", "0"),
                row("USDT0", USDT, "80", "0", "0"),
                row("HYPE", 150, "10", "0", "0"),
            ],
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

fn usable(standing: &Standing, surface: Surface<'_>) -> Usable {
    standing.usable(surface, &collateral(), hype_at_40)
}

#[test]
fn standard_draws_each_dex_on_its_own_and_spot_on_spot() {
    let standing = standing(Abstraction::Disabled);
    let xyz = usable(&standing, Surface::Perps { dex: "xyz" });
    assert_eq!((xyz.amount, xyz.token), (dec("25"), USDC));
    assert_eq!(xyz.from.words(), "perps on xyz");
    assert_eq!(usable(&standing, Surface::Perps { dex: "" }).amount, dec("100"));
    let spot = usable(&standing, Surface::Spot { quote: USDC });
    assert_eq!(spot.amount, dec("380"), "the hold is not free");
    assert_eq!(spot.from.words(), "spot");
    assert_eq!(usable(&standing, Surface::Outcomes), spot, "outcomes draw on spot USDC as spot does");
    assert_eq!(usable(&standing, Surface::Spot { quote: USDT }).amount, dec("80"), "a USDT pair on USDT");
    assert_eq!(usable(&standing, Surface::Perps { dex: "cash" }).amount, Dec::ZERO, "a dex never read has nothing");
}

#[test]
fn unified_draws_every_dex_on_the_spot_balance_of_its_collateral() {
    let standing = standing(Abstraction::UnifiedAccount);
    let xyz = usable(&standing, Surface::Perps { dex: "xyz" });
    assert_eq!(xyz.amount, dec("380"), "USDC in spot, not xyz's own state");
    assert_eq!((xyz.token, xyz.from.words()), (USDC, "unified, in spot".to_owned()));
    let cash = usable(&standing, Surface::Perps { dex: "cash" });
    assert_eq!((cash.amount, cash.token), (dec("80"), USDT), "cash is backed by USDT");
    assert_eq!(usable(&standing, Surface::Spot { quote: USDC }).amount, dec("380"), "and spot draws on the same");
    assert_eq!(usable(&standing, Surface::Outcomes).amount, dec("380"));
}

#[test]
fn portfolio_margin_adds_what_it_would_lend_against_hype() {
    let mut standing = standing(Abstraction::PortfolioMargin);
    // 10 HYPE at 40 is 400, and 0.65 of that is 260.
    let main = usable(&standing, Surface::Perps { dex: "" });
    assert_eq!((main.amount, main.borrowable), (dec("640"), dec("260")));
    assert_eq!(main.from.words(), "portfolio margin");
    if let Some(spot) = standing.spot.as_mut() {
        spot.balances[0].borrowed = Some(dec("60"));
    }
    assert_eq!(usable(&standing, Surface::Spot { quote: USDC }).amount, dec("580"), "less what is borrowed already");
    assert_eq!(usable(&standing, Surface::Perps { dex: "cash" }).amount, dec("80"), "USDT borrows nothing here");
    if let Some(spot) = standing.spot.as_mut() {
        spot.balances.push(row("UBTC", 197, "0.01", "0", "0"));
        spot.balances[0].borrowed = None;
    }
    // 0.01 BTC at 80000 is 800, and half of that is 400, on top of 380 free
    // USDC and 260 against HYPE.
    assert_eq!(usable(&standing, Surface::Spot { quote: USDC }).amount, dec("1040"));
}

#[test]
fn portfolio_margin_lends_at_each_row_s_own_ltv_up_to_what_maintenance_leaves() {
    let mut standing = standing(Abstraction::PortfolioMargin);
    if let Some(spot) = standing.spot.as_mut() {
        spot.balances[2].ltv = Some(dec("0.5"));
        spot.token_to_available_after_maintenance = vec![(USDC, dec("150")), (150, dec("10"))];
    }
    let main = usable(&standing, Surface::Perps { dex: "" });
    assert_eq!(main.borrowable, dec("150"), "200 at the row's own 0.5, held to the 150 after maintenance");
    if let Some(spot) = standing.spot.as_mut() {
        spot.portfolio_margin_ratio = Some(dec("0.96"));
    }
    assert_eq!(usable(&standing, Surface::Perps { dex: "" }).amount, dec("380"), "liquidatable: nothing lent");
}

/// Portfolio margin, live 2026-10-06 (`tests.rs`): the venue's own room to
/// trade, `total - hold` of USDC (0.55817531 + 268.81821068), is what HYPE
/// lends at 0.65, less the 89.3303277 owed. HYPE/USDC traded between 91.375
/// and 92.791 in the two hours around the read (`candleSnapshot` of `@107`).
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

#[test]
fn dex_abstraction_draws_a_usdc_dex_on_the_main_dex() {
    let standing = standing(Abstraction::DexAbstraction);
    let xyz = usable(&standing, Surface::Perps { dex: "xyz" });
    assert_eq!(xyz.amount, dec("100"), "the main dex's USDC");
    assert_eq!(xyz.from.words(), "perps, the main dex's USDC");
    assert_eq!(usable(&standing, Surface::Perps { dex: "" }).from.words(), "perps");
    assert_eq!(usable(&standing, Surface::Perps { dex: "cash" }).amount, Dec::ZERO, "a USDT dex keeps its own");
    assert_eq!(usable(&standing, Surface::Spot { quote: USDC }).amount, dec("380"));
}

#[test]
fn a_mode_not_read_is_standard_unless_spot_says_portfolio_margin() {
    let mut standing = standing(Abstraction::Disabled);
    standing.mode = None;
    assert!(!standing.pools());
    assert_eq!(usable(&standing, Surface::Perps { dex: "xyz" }).amount, dec("25"));
    if let Some(spot) = standing.spot.as_mut() {
        spot.portfolio_margin_enabled = Some(true);
    }
    assert!(standing.pools());
    assert_eq!(standing.keeping(), Keeping::Portfolio);
}

fn recorded(spot: &str, main: &str, xyz: &str, mode: Abstraction) -> Standing {
    Standing {
        mode: Some(mode),
        dexes: vec![
            DexState { dex: String::new(), state: read(main) },
            DexState { dex: "xyz".to_owned(), state: read(xyz) },
        ],
        spot: Some(read(spot)),
        heard: None,
    }
}

/// A live unified account: every dex draws on spot USDC less its hold, and
/// the main dex's own `withdrawable` (7046.148664) is no balance of its own.
#[test]
fn a_live_unified_account_draws_everything_on_spot() {
    let standing = recorded(SPOT_UNIFIED, PERP_UNIFIED, PERP_UNIFIED_XYZ, Abstraction::UnifiedAccount);
    let free = dec("736222.0525745799");
    for surface in
        [Surface::Perps { dex: "" }, Surface::Perps { dex: "xyz" }, Surface::Spot { quote: USDC }, Surface::Outcomes]
    {
        assert_eq!(usable(&standing, surface).amount, free, "{surface:?}");
    }
    assert_eq!(usable(&standing, Surface::Perps { dex: "cash" }).amount, Dec::ZERO, "no USDT0 held");
    let balances = standing.balances(&collateral());
    assert_eq!(
        balances,
        [Balance { token: USDC, total: dec("1279260.79987858"), available: free, pays: Pays::Any }],
        "one balance; the empty stablecoins and the HYPE beside it are none"
    );
}

/// A live standard account: each ledger its own.
#[test]
fn a_live_standard_account_draws_each_ledger_on_its_own() {
    let standing = recorded(SPOT_STANDARD, PERP_STANDARD, PERP_STANDARD_XYZ, Abstraction::Default);
    assert_eq!(usable(&standing, Surface::Perps { dex: "" }).amount, dec("463.943191"));
    assert_eq!(usable(&standing, Surface::Perps { dex: "xyz" }).amount, Dec::ZERO);
    assert_eq!(usable(&standing, Surface::Spot { quote: USDC }).amount, dec("246.63995144"));
    assert_eq!(usable(&standing, Surface::Outcomes).amount, dec("246.63995144"));
    let balances = standing.balances(&collateral());
    assert_eq!(
        balances,
        [
            Balance {
                token: USDC,
                total: dec("463.943191"),
                available: dec("463.943191"),
                pays: Pays::Perps { dex: String::new() }
            },
            Balance {
                token: USDC,
                total: Dec::ZERO,
                available: Dec::ZERO,
                pays: Pays::Perps { dex: "xyz".to_owned() }
            },
            Balance { token: USDC, total: dec("246.63995144"), available: dec("246.63995144"), pays: Pays::Spot },
        ]
    );
    let xyz = payer(&balances, Surface::Perps { dex: "xyz" }, &collateral());
    assert_eq!(xyz.map(|balance| &balance.pays), Some(&Pays::Perps { dex: "xyz".to_owned() }));
    assert_eq!(payer(&balances, Surface::Perps { dex: "flx" }, &collateral()), None, "USDH on flx: none read");
}

#[test]
fn a_pooled_account_moves_nothing_between_perps_and_spot() {
    let class = |to_perp| Move::ClassTransfer { to_perp };
    for keeping in [Keeping::Apart, Keeping::Native] {
        assert_eq!(keeping.draws(class(true)), Draws::Spot, "{keeping:?}");
        assert_eq!(keeping.draws(class(false)), Draws::Perp, "{keeping:?}");
        assert_eq!(keeping.draws(Move::Send), Draws::Perp, "usdSend spends the perp balance");
        assert_eq!(keeping.draws(Move::Withdraw), Draws::Perp, "and withdraw3 too");
        assert_eq!(keeping.refuses(class(true)), None);
    }
    for (keeping, account) in
        [(Keeping::Unified, "a unified account"), (Keeping::Portfolio, "a portfolio margin account")]
    {
        for kind in [class(true), class(false), Move::Send, Move::Withdraw] {
            assert_eq!(keeping.draws(kind), Draws::Pooled, "{keeping:?} {kind:?}");
        }
        assert_eq!(
            keeping.refuses(class(false)),
            Some(format!("{account} holds USDC in one balance, so there is nothing to move between perps and spot"))
        );
        assert_eq!(keeping.refuses(Move::Send), None);
    }
}

#[test]
fn only_followed_users_are_kept_and_at_most_ten() {
    let user = |tail: u8| hl_core::Address::from_bytes([tail; 20]);
    let mut standings = Standings::default();
    let many: Vec<_> = (1..=12).chain([1]).map(user).collect();
    assert!(standings.follow(&many));
    assert_eq!(standings.followed().len(), MOST_USERS);
    assert_eq!(standings.followed()[0], user(1), "the first kept first");
    assert!(!standings.follow(&many), "the same users again change nothing");

    assert!(!standings.accept(user(12), 5, |standing| standing.dexes = vec![dex("", "1")]), "not followed, not kept");
    assert!(standings.of(user(12)).is_none());
    assert!(standings.accept(user(1), 5, |standing| standing.dexes = vec![dex("", "1")]));
    assert_eq!(standings.of(user(1)).and_then(|one| one.heard), Some(5));

    standings.accept_mode(user(1), None, 6);
    standings.accept_mode(user(1), Some(Abstraction::UnifiedAccount), 7);
    standings.accept_mode(user(1), None, 8);
    assert_eq!(
        standings.of(user(1)).and_then(|one| one.mode.clone()),
        Some(Abstraction::UnifiedAccount),
        "a frame that says no mode keeps the one known"
    );

    standings.follow(&[user(2)]);
    assert!(standings.of(user(1)).is_none(), "one no longer followed is let go");
}
