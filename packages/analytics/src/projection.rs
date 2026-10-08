//! The revenue projection: what a HIP-4 deployer, builder and referrer could earn as traders
//! come and trade, period by period. hip4-tui's model and its rows, without the page.
//!
//! Every figure is an integer in a stated unit, as hip4-tui keeps them: money in cents, rates in
//! hundredths of a basis point ([`PER_RATE`]), shares and growth in hundredths of a percent
//! ([`PER_SHARE`]), traders scaled by [`PER_USER`] so growth does not round to a person. The fee
//! model is D11's: a trader pays `base * (scale + max(scale, 1))` on the volume that closes or
//! settles, and the deployer takes the `scale` part of it; a builder is paid on all volume, buys
//! included; a referrer takes its cut of what its traders pay.

pub mod rows;

pub use rows::{Row, Source, Unit};

use crate::date::{At, civil, month_name};

/// Hundredths of a basis point in one whole: a rate of 1 is a millionth.
pub const PER_RATE: u128 = 1_000_000;

/// Hundredths of a percent in one whole.
pub const PER_SHARE: u128 = 10_000;

/// What the trader count is scaled by, so growth does not round to a person.
pub const PER_USER: u128 = 10_000;

/// The most periods projected: five years of months.
pub const MAX_PERIODS: u32 = 60;

/// The highest fee scale, in hundredths: the venue allows up to 10.
pub const MAX_SCALE: u32 = 1_000;

/// How long a period is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Week,
    Month,
    Quarter,
}

impl Step {
    pub const ALL: [Step; 3] = [Step::Week, Step::Month, Step::Quarter];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Step::Week => "week",
            Step::Month => "month",
            Step::Quarter => "quarter",
        }
    }

    #[must_use]
    pub const fn plural(self) -> &'static str {
        match self {
            Step::Week => "weeks",
            Step::Month => "months",
            Step::Quarter => "quarters",
        }
    }

    /// Periods in a year.
    #[must_use]
    pub const fn a_year(self) -> u32 {
        match self {
            Step::Week => 52,
            Step::Month => 12,
            Step::Quarter => 4,
        }
    }

    /// The next step, round again after a quarter.
    #[must_use]
    pub const fn next(self) -> Step {
        match self {
            Step::Week => Step::Month,
            Step::Month => Step::Quarter,
            Step::Quarter => Step::Week,
        }
    }

    /// `periods` of this step on from `from`, or back; a month keeps its day where the month has
    /// it, and its last day where it does not.
    #[must_use]
    pub fn after(self, from: At, periods: i32) -> At {
        let periods = i64::from(periods);
        match self {
            Step::Week => from.plus_days(periods * 7),
            Step::Month => from.plus_months(periods),
            Step::Quarter => from.plus_months(periods * 3),
        }
    }

    /// What a period starting at `at` is called: `15 Sep`, `Nov 2026`, `Q4 2026`.
    #[must_use]
    pub fn name(self, at: At) -> String {
        let (year, month, day) = civil(at);
        match self {
            Step::Week => format!("{day:02} {}", month_name(month)),
            Step::Month => format!("{} {year}", month_name(month)),
            Step::Quarter => format!("Q{} {year}", (month - 1) / 3 + 1),
        }
    }
}

/// How revenue is worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Each fee on its own: the deployer's, the builder's and the referrer's.
    ByFee,
    /// One blended rate on the volume that pays.
    Total,
}

impl Mode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Mode::ByFee => "by fee",
            Mode::Total => "one rate",
        }
    }

    #[must_use]
    pub const fn next(self) -> Mode {
        match self {
            Mode::ByFee => Mode::Total,
            Mode::Total => Mode::ByFee,
        }
    }
}

/// The assumptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Projection {
    /// Periods from now to the first, either way.
    pub start: i32,
    pub step: Step,
    /// Periods projected, at most [`MAX_PERIODS`].
    pub horizon: u32,
    /// Traders in the first period.
    pub users: u64,
    /// Per period, in hundredths of a percent; below zero, a decline.
    pub growth: i32,
    /// Traded per trader per period, in cents.
    pub spend: u128,
    /// Of volume, the part that pays a fee (closes and settles), in hundredths of a percent.
    pub paying: u32,
    pub mode: Mode,
    /// [`Mode::Total`]'s rate on the volume that pays, in hundredths of a basis point.
    pub blended: u32,
    /// The base outcome rate a trader pays before the scale, in hundredths of a basis point.
    pub base: u32,
    /// The market's `deployerFeeScale`, in hundredths: 100 is the `1` that doubles what is paid.
    pub scale: u32,
    /// Of volume, the part on the deployer's own markets, in hundredths of a percent.
    pub deployer_reach: u32,
    /// The builder fee, in hundredths of a basis point.
    pub builder_fee: u32,
    /// Of volume, the part carrying the builder's code, in hundredths of a percent.
    pub builder_reach: u32,
    /// The referrer's cut of what a referred trader pays, in hundredths of a percent.
    pub referral_share: u32,
    /// Of volume, the part from traders the referrer signed up, in hundredths of a percent.
    pub referral_reach: u32,
}

/// hip4-tui's starting assumptions: the rate the venue charged on 2026-09-12 (6.72 bp at a scale
/// of 1, 13.44 bp paid), monthly over a year.
impl Default for Projection {
    fn default() -> Self {
        Projection {
            start: 0,
            step: Step::Month,
            horizon: 12,
            users: 2_500,
            growth: 1_500,
            spend: 250_000,
            paying: 5_000,
            mode: Mode::ByFee,
            blended: 500,
            base: 672,
            scale: 100,
            deployer_reach: 10_000,
            builder_fee: 0,
            builder_reach: 10_000,
            referral_share: 1_000,
            referral_reach: 5_000,
        }
    }
}

/// A horizon summed, or a year at its last period's rate. Money in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    /// Where the count ends, scaled by [`PER_USER`].
    pub users: u128,
    pub volume: u128,
    pub paying: u128,
    pub charged: u128,
    pub deployer: u128,
    pub builder: u128,
    pub referral: u128,
    pub revenue: u128,
}

/// One period. Money in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub at: At,
    /// Traders, scaled by [`PER_USER`].
    pub users: u128,
    pub volume: u128,
    /// The volume that pays a fee.
    pub paying: u128,
    /// What traders pay on it in all, the deployer's part included.
    pub charged: u128,
    pub deployer: u128,
    pub builder: u128,
    pub referral: u128,
    pub revenue: u128,
    /// Revenue to the end of this period.
    pub cumulative: u128,
}

impl Projection {
    /// The horizon from `now`, period by period.
    #[must_use]
    pub fn periods(&self, now: At) -> Vec<Period> {
        let first = self.step.after(now, self.start);
        let mut people = u128::from(self.users).saturating_mul(PER_USER);
        let mut running = 0u128;
        let horizon = self.horizon.min(MAX_PERIODS);
        let mut out = Vec::with_capacity(usize::try_from(horizon).unwrap_or(0));
        for index in 0..horizon {
            let volume = people.saturating_mul(self.spend) / PER_USER;
            let paying = share(volume, self.paying, WHOLE);
            let charged = rate(paying, self.user_rate());
            let (deployer, builder, referral) = match self.mode {
                Mode::Total => (0, 0, 0),
                Mode::ByFee => (
                    share(rate(paying, self.deployer_rate()), self.deployer_reach, WHOLE),
                    share(rate(volume, self.builder_fee), self.builder_reach, WHOLE),
                    share(charged, self.referral_reach, self.referral_share),
                ),
            };
            let revenue = match self.mode {
                Mode::Total => rate(paying, self.blended),
                Mode::ByFee => deployer.saturating_add(builder).saturating_add(referral),
            };
            running = running.saturating_add(revenue);
            out.push(Period {
                at: self.step.after(first, i32::try_from(index).unwrap_or(0)),
                users: people,
                volume,
                paying,
                charged,
                deployer,
                builder,
                referral,
                revenue,
                cumulative: running,
            });
            people = grown(people, self.growth);
        }
        out
    }

    /// What a trader pays, in hundredths of a basis point: the base times the scale plus what
    /// the protocol keeps, at least the base.
    #[must_use]
    pub const fn user_rate(&self) -> u32 {
        let scale = self.scale as u64;
        let protocol = if scale > 100 { scale } else { 100 };
        let rate = (self.base as u64).saturating_mul(scale + protocol) / 100;
        if rate > u32::MAX as u64 { u32::MAX } else { rate as u32 }
    }

    /// The deployer's part of [`Projection::user_rate`].
    #[must_use]
    pub const fn deployer_rate(&self) -> u32 {
        let rate = (self.base as u64).saturating_mul(self.scale as u64) / 100;
        if rate > u32::MAX as u64 { u32::MAX } else { rate as u32 }
    }

    /// What traders pay on `volume` cents.
    #[must_use]
    pub fn charged_on(&self, volume: u128) -> u128 {
        rate(share(volume, self.paying, WHOLE), self.user_rate())
    }

    #[must_use]
    pub fn totals(periods: &[Period]) -> Totals {
        let mut totals = periods.iter().fold(Totals::default(), |sum, one| Totals {
            users: 0,
            volume: sum.volume.saturating_add(one.volume),
            paying: sum.paying.saturating_add(one.paying),
            charged: sum.charged.saturating_add(one.charged),
            deployer: sum.deployer.saturating_add(one.deployer),
            builder: sum.builder.saturating_add(one.builder),
            referral: sum.referral.saturating_add(one.referral),
            revenue: sum.revenue.saturating_add(one.revenue),
        });
        totals.users = periods.last().map_or(0, |last| last.users);
        totals
    }

    /// A year at the last period's rate.
    #[must_use]
    pub fn a_year(periods: &[Period], step: Step) -> Totals {
        let Some(last) = periods.last() else {
            return Totals::default();
        };
        let over = |amount: u128| amount.saturating_mul(u128::from(step.a_year()));
        Totals {
            users: last.users,
            volume: over(last.volume),
            paying: over(last.paying),
            charged: over(last.charged),
            deployer: over(last.deployer),
            builder: over(last.builder),
            referral: over(last.referral),
            revenue: over(last.revenue),
        }
    }

    /// One source's revenue, period by period.
    #[must_use]
    pub fn series(source: Source, periods: &[Period]) -> Vec<u128> {
        periods
            .iter()
            .map(|one| match source {
                Source::All => one.revenue,
                Source::Builder => one.builder,
                Source::Deployer => one.deployer,
                Source::Referral => one.referral,
            })
            .collect()
    }
}

/// [`PER_SHARE`] as a share's own type: all of it.
const WHOLE: u32 = 10_000;

/// `amount` at `rate` hundredths of a basis point, cut down.
fn rate(amount: u128, rate: u32) -> u128 {
    amount.saturating_mul(u128::from(rate)) / PER_RATE
}

/// `of_it` of the `reach` of `amount`, both in hundredths of a percent, cut down.
fn share(amount: u128, reach: u32, of_it: u32) -> u128 {
    amount.saturating_mul(u128::from(reach)).saturating_mul(u128::from(of_it)) / (PER_SHARE * PER_SHARE)
}

/// One period's growth on `people`; a decline stops at nobody.
fn grown(people: u128, growth: i32) -> u128 {
    let factor = (i128::from(WHOLE) + i128::from(growth)).max(0);
    people.saturating_mul(factor.unsigned_abs()) / PER_SHARE
}

/// The least-squares line through `values`, at each of their places, in integers; never below
/// zero. Fewer than two values are their own line.
#[must_use]
pub fn trend(values: &[u128]) -> Vec<u128> {
    let Ok(n) = i128::try_from(values.len()) else {
        return values.to_vec();
    };
    if n < 2 {
        return values.to_vec();
    }
    let sum_x = n * (n - 1) / 2;
    let sum_x2 = (n - 1) * n * (2 * n - 1) / 6;
    let (mut sum_y, mut sum_xy) = (0i128, 0i128);
    for (x, value) in (0i128..).zip(values) {
        let y = i128::try_from(*value).unwrap_or(i128::MAX);
        sum_y = sum_y.saturating_add(y);
        sum_xy = sum_xy.saturating_add(y.saturating_mul(x));
    }
    let den = n * sum_x2 - sum_x * sum_x;
    let num = n.saturating_mul(sum_xy).saturating_sub(sum_x.saturating_mul(sum_y));
    (0i128..n)
        .map(|x| {
            let fitted = sum_y.saturating_mul(den).saturating_add(num.saturating_mul(x * n - sum_x)) / (n * den);
            u128::try_from(fitted.max(0)).unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/projection.rs"]
mod tests;
