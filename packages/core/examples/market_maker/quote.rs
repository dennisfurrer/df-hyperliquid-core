//! Where the two quotes go, as a pure function of the touch, the position and the settings: no
//! clock, no network. Prices land on the venue's grid with exact decimals, a set number of ticks
//! (0 or more) behind the best price on their own side, so a post-only quote cannot cross; at 0 a
//! quote may join the best bid or ask. Sizes are cut to the lot and raised to the venue's $10
//! minimum when they fall under it.

use df_hyperliquid_core::Dec;
use df_hyperliquid_core::markets::Side;
use df_hyperliquid_core::utils::Round;
use df_hyperliquid_core::utils::rounding::{Grid, SIGNIFICANT_FIGURES};

/// Basis points in one.
const BPS: Dec = Dec::from_int(10_000);

/// What the quotes depend on besides the book.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// The coin's grid: its size decimals and price places.
    pub grid: Grid,
    /// Half the spread around the centre, in basis points of the mid.
    pub half_spread_bps: Dec,
    /// Each quote's size, in the base asset.
    pub size: Dec,
    /// How far the centre moves at the full position, in basis points of the mid: long moves it
    /// down, short moves it up, so the side that reduces the position is the likelier to fill.
    pub skew_bps: Dec,
    /// The largest position, either way, in the base asset. At it, the side that would add to
    /// the position is not quoted.
    pub max_position: Dec,
    /// How many ticks each quote stays behind the best price on its own side: 0 lets a quote
    /// join the best bid or ask, which a post-only order may do without crossing.
    pub behind_ticks: Dec,
}

/// One quote: a price and a size, both on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    /// The limit price.
    pub px: Dec,
    /// The size, in the base asset.
    pub sz: Dec,
}

/// The two quotes; a side is `None` when it is not quoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Quotes {
    /// The bid.
    pub bid: Option<Level>,
    /// The ask.
    pub ask: Option<Level>,
    /// Whether a size was raised to the venue's $10 minimum.
    pub raised_to_minimum: bool,
}

impl Quotes {
    /// The quote on `side`.
    pub fn on(&self, side: Side) -> Option<Level> {
        match side {
            Side::Buy => self.bid,
            Side::Sell => self.ask,
        }
    }
}

/// The price step at `price` on `grid`: one unit in the last place the venue takes there.
pub fn tick(grid: Grid, price: Dec) -> Option<Dec> {
    let places = grid.price_places().min(price.sig_places(SIGNIFICANT_FIGURES));
    Dec::new(1, places)
}

/// The two quotes for a book whose best bid and ask are `best_bid` and `best_ask`, holding
/// `position` (positive long, negative short). `None` when the arithmetic leaves the decimal range
/// or the book is crossed.
pub fn quotes(settings: &Settings, best_bid: Dec, best_ask: Dec, position: Dec) -> Option<Quotes> {
    if best_bid >= best_ask {
        return None;
    }
    let mid = best_bid.checked_add(best_ask)?.checked_div(Dec::from_int(2), 12, Round::Nearest)?;
    // The share of the largest position held, between -1 and 1.
    let held = if settings.max_position.is_positive() {
        position.checked_div(settings.max_position, 12, Round::Nearest)?
    } else {
        Dec::ZERO
    };
    let held = held.max(Dec::from_int(-1)).min(Dec::ONE);
    let shift = mid.checked_mul(settings.skew_bps)?.checked_mul(held)?.checked_div(BPS, 12, Round::Nearest)?;
    let centre = mid.checked_sub(shift)?;
    let half = centre.checked_mul(settings.half_spread_bps)?.checked_div(BPS, 12, Round::Nearest)?;

    let mut out = Quotes::default();
    // The bid: rounded down onto the grid, then kept `behind_ticks` under the best bid.
    if position < settings.max_position {
        let wanted = settings.grid.price(centre.checked_sub(half)?, Side::Buy)?;
        let ceiling = best_bid.checked_sub(tick(settings.grid, best_bid)?.checked_mul(settings.behind_ticks)?)?;
        let px = settings.grid.price(wanted.min(ceiling), Side::Buy)?;
        out.bid = sized(settings, px, &mut out.raised_to_minimum);
    }
    // The ask: rounded up onto the grid, then kept `behind_ticks` over the best ask.
    if position > settings.max_position.negate() {
        let wanted = settings.grid.price(centre.checked_add(half)?, Side::Sell)?;
        let floor = best_ask.checked_add(tick(settings.grid, best_ask)?.checked_mul(settings.behind_ticks)?)?;
        let px = settings.grid.price(wanted.max(floor), Side::Sell)?;
        out.ask = sized(settings, px, &mut out.raised_to_minimum);
    }
    Some(out)
}

/// The quote at `px`: the size cut to the lot, raised to the venue's minimum when it falls under.
fn sized(settings: &Settings, px: Dec, raised: &mut bool) -> Option<Level> {
    let mut sz = settings.grid.size(settings.size).unwrap_or(Dec::ZERO);
    if !Grid::meets_minimum(px, sz) {
        sz = settings.grid.least_size(px)?;
        *raised = true;
    }
    Some(Level { px, sz })
}

/// Whether the resting quote `now` should be replaced by `want`: a price a tick or more away, or
/// another size.
pub fn needs_change(grid: Grid, now: Level, want: Level) -> bool {
    if now.sz != want.sz {
        return true;
    }
    let Some(step) = tick(grid, want.px) else { return true };
    let gap = now.px.checked_sub(want.px).map_or(step, Dec::abs);
    gap >= step
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Dec {
        text.parse().expect("a decimal")
    }

    /// BTC on the main dex: five size decimals, so whole-dollar prices above 10,000.
    fn btc() -> Settings {
        Settings {
            grid: Grid::perp(5),
            half_spread_bps: dec("2"),
            size: dec("0.0002"),
            skew_bps: dec("5"),
            max_position: dec("0.001"),
            behind_ticks: Dec::ONE,
        }
    }

    #[test]
    fn tick_follows_the_grid() {
        assert_eq!(tick(Grid::perp(5), dec("91234")), Some(dec("1")));
        assert_eq!(tick(Grid::perp(5), dec("9123.4")), Some(dec("0.1")));
        assert_eq!(tick(Grid::perp(0), dec("1.2345")), Some(dec("0.0001")));
    }

    #[test]
    fn quotes_sit_on_the_grid_and_behind_the_touch() {
        let q = quotes(&btc(), dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        let (bid, ask) = (q.bid.expect("a bid"), q.ask.expect("an ask"));
        assert!(Grid::perp(5).is_on_grid(bid.px) && Grid::perp(5).is_on_grid(ask.px));
        assert!(bid.px <= dec("90999"), "bid {}", bid.px);
        assert!(ask.px >= dec("91002"), "ask {}", ask.px);
        // 2 bps of 91,000.5 is 18.2: the bid rounds down, the ask up.
        assert_eq!(bid.px, dec("90982"));
        assert_eq!(ask.px, dec("91019"));
    }

    #[test]
    fn a_tight_spread_still_stays_a_tick_behind() {
        let settings = Settings { half_spread_bps: Dec::ZERO, ..btc() };
        let q = quotes(&settings, dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        assert_eq!(q.bid.map(|level| level.px), Some(dec("90999")));
        assert_eq!(q.ask.map(|level| level.px), Some(dec("91002")));
    }

    #[test]
    fn zero_ticks_behind_joins_the_touch_without_crossing() {
        let settings = Settings { half_spread_bps: Dec::ZERO, behind_ticks: Dec::ZERO, ..btc() };
        let q = quotes(&settings, dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        assert_eq!(q.bid.map(|level| level.px), Some(dec("91000")));
        assert_eq!(q.ask.map(|level| level.px), Some(dec("91001")));
        let three = Settings { behind_ticks: dec("3"), ..settings };
        let q = quotes(&three, dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        assert_eq!(q.bid.map(|level| level.px), Some(dec("90997")));
        assert_eq!(q.ask.map(|level| level.px), Some(dec("91004")));
    }

    #[test]
    fn a_long_position_moves_both_quotes_down() {
        let flat = quotes(&btc(), dec("91000"), dec("91001"), Dec::ZERO).expect("flat");
        let long = quotes(&btc(), dec("91000"), dec("91001"), dec("0.0005")).expect("long");
        assert!(long.bid.expect("bid").px < flat.bid.expect("bid").px);
        assert!(long.ask.expect("ask").px <= flat.ask.expect("ask").px);
    }

    #[test]
    fn the_position_limit_drops_the_side_that_adds() {
        let long = quotes(&btc(), dec("91000"), dec("91001"), dec("0.001")).expect("quotes");
        assert!(long.bid.is_none() && long.ask.is_some());
        let short = quotes(&btc(), dec("91000"), dec("91001"), dec("-0.001")).expect("quotes");
        assert!(short.ask.is_none() && short.bid.is_some());
    }

    #[test]
    fn a_size_under_the_minimum_is_raised_to_it() {
        let settings = Settings { size: dec("0.00001"), ..btc() };
        let q = quotes(&settings, dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        let bid = q.bid.expect("a bid");
        assert!(q.raised_to_minimum);
        assert!(Grid::meets_minimum(bid.px, bid.sz), "{} at {}", bid.sz, bid.px);
        assert_eq!(bid.sz, dec("0.00011"));
    }

    #[test]
    fn a_size_is_cut_to_the_lot() {
        let settings = Settings { size: dec("0.000234567"), ..btc() };
        let q = quotes(&settings, dec("91000"), dec("91001"), Dec::ZERO).expect("quotes");
        assert_eq!(q.bid.expect("a bid").sz, dec("0.00023"));
    }

    #[test]
    fn a_crossed_book_gives_nothing() {
        assert!(quotes(&btc(), dec("91001"), dec("91000"), Dec::ZERO).is_none());
    }

    #[test]
    fn a_change_is_a_tick_or_another_size() {
        let grid = Grid::perp(5);
        let now = Level { px: dec("91000"), sz: dec("0.0002") };
        assert!(!needs_change(grid, now, now));
        assert!(needs_change(grid, now, Level { px: dec("91001"), ..now }));
        assert!(needs_change(grid, now, Level { sz: dec("0.0003"), ..now }));
    }
}
