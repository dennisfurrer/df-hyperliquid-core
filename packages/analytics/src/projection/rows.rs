//! The assumptions a projection is made of, one row each: what it is called and means, the unit
//! a typed figure is read in, how far one step moves it and the bounds it is held to.

use super::{MAX_PERIODS, MAX_SCALE, Mode, PER_RATE, PER_SHARE, Projection};

/// The unit a row's figure is typed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A whole number.
    Count,
    /// A percent to two places: `7.5` is 750.
    Percent,
    /// Basis points to two places: `6.72` is 672.
    Rate,
    /// Dollars to the cent: `1250.50` is 125050.
    Money,
}

impl Unit {
    /// A typed figure in this unit, in the unit's integer; places past it are cut. `None` for
    /// anything but digits and one point.
    #[must_use]
    pub fn read(self, text: &str) -> Option<u128> {
        let places = match self {
            Unit::Count => 0,
            Unit::Percent | Unit::Rate | Unit::Money => 2,
        };
        fixed(text, places)
    }
}

/// Where revenue comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    All,
    Builder,
    Deployer,
    Referral,
}

impl Source {
    pub const ALL: [Source; 4] = [Source::All, Source::Builder, Source::Deployer, Source::Referral];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Source::All => "All",
            Source::Builder => "Builder Fees",
            Source::Deployer => "Deployer Fees",
            Source::Referral => "Referral Fees",
        }
    }
}

/// One assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Start,
    Step,
    Horizon,
    Users,
    Growth,
    Spend,
    Paying,
    Mode,
    Blended,
    Base,
    Scale,
    DeployerReach,
    BuilderFee,
    BuilderReach,
    ReferralShare,
    ReferralReach,
}

impl Row {
    pub const ALL: [Row; 16] = [
        Row::Start,
        Row::Step,
        Row::Horizon,
        Row::Users,
        Row::Growth,
        Row::Spend,
        Row::Paying,
        Row::Mode,
        Row::Blended,
        Row::Base,
        Row::Scale,
        Row::DeployerReach,
        Row::BuilderFee,
        Row::BuilderReach,
        Row::ReferralShare,
        Row::ReferralReach,
    ];

    #[must_use]
    pub const fn group(self) -> &'static str {
        match self {
            Row::Start | Row::Step | Row::Horizon => "when",
            Row::Users | Row::Growth | Row::Spend | Row::Paying => "traders",
            _ => "fees",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Row::Start => "start",
            Row::Step => "period",
            Row::Horizon => "horizon",
            Row::Users => "traders",
            Row::Growth => "growth",
            Row::Spend => "traded each",
            Row::Paying => "pays a fee",
            Row::Mode => "fees",
            Row::Blended => "one rate",
            Row::Base => "base rate",
            Row::Scale => "deployer scale",
            Row::DeployerReach => "on your markets",
            Row::BuilderFee => "builder fee",
            Row::BuilderReach => "through your app",
            Row::ReferralShare => "referral cut",
            Row::ReferralReach => "signed up by you",
        }
    }

    #[must_use]
    pub const fn hint(self) -> &'static str {
        match self {
            Row::Start => "when the first period begins",
            Row::Step => "week, month or quarter",
            Row::Horizon => "how many periods are projected",
            Row::Users => "how many trade in the first period",
            Row::Growth => "compounded, period on period",
            Row::Spend => "notional one trader does in a period",
            Row::Paying => "of volume; only closes and settles pay, makers never do",
            Row::Mode => "each fee on its own, or one rate on the volume that pays",
            Row::Blended => "of fee-paying volume, when the split is not the question",
            Row::Base => "the outcome rate before the scale, as the venue quotes it",
            Row::Scale => "deployerFeeScale, 0 to 10; the deployer takes the scale part",
            Row::DeployerReach => "of volume; a scale is set per market, so not all of it",
            Row::BuilderFee => "of all volume, buys included; zero until you turn it on",
            Row::BuilderReach => "of volume carrying your builder code",
            Row::ReferralShare => "of what a referred trader pays; the venue pays 10%",
            Row::ReferralReach => "of volume; not everyone signs up under a code",
        }
    }

    /// The unit a typed figure is read in; `None` for a choice.
    #[must_use]
    pub const fn unit(self) -> Option<Unit> {
        match self {
            Row::Step | Row::Mode => None,
            Row::Start | Row::Horizon | Row::Users => Some(Unit::Count),
            Row::Growth
            | Row::Paying
            | Row::DeployerReach
            | Row::BuilderReach
            | Row::ReferralShare
            | Row::ReferralReach
            | Row::Scale => Some(Unit::Percent),
            Row::Blended | Row::Base | Row::BuilderFee => Some(Unit::Rate),
            Row::Spend => Some(Unit::Money),
        }
    }

    /// Whether the mode reads this row.
    #[must_use]
    pub const fn live(self, projection: &Projection) -> bool {
        match (projection.mode, self) {
            (Mode::Total, Row::Blended) => true,
            (Mode::Total, row) => !matches!(
                row,
                Row::Base
                    | Row::Scale
                    | Row::DeployerReach
                    | Row::BuilderFee
                    | Row::BuilderReach
                    | Row::ReferralShare
                    | Row::ReferralReach
            ),
            (Mode::ByFee, row) => !matches!(row, Row::Blended),
        }
    }

    /// How far one step moves the figure, in its unit.
    #[must_use]
    pub const fn stride(self) -> i64 {
        match self {
            Row::Start | Row::Horizon => 1,
            Row::Users => 100,
            Row::Growth => 25,
            Row::Spend => 10_000,
            Row::Blended | Row::Base | Row::BuilderFee => 25,
            Row::Scale => 25,
            Row::Paying | Row::DeployerReach | Row::BuilderReach | Row::ReferralShare | Row::ReferralReach => 100,
            Row::Step | Row::Mode => 1,
        }
    }

    /// Moves the figure `steps` strides, held to what it can mean; a choice moves to the next
    /// option, or back.
    pub fn bump(self, projection: &mut Projection, steps: i64) {
        let by = self.stride().saturating_mul(steps);
        let share = |value: &mut u32| *value = moved(*value, by, PER_SHARE);
        let rate = |value: &mut u32| *value = moved(*value, by, PER_RATE);
        match self {
            Row::Start => projection.start = clamped(i64::from(projection.start).saturating_add(by), -24, 24),
            Row::Step if steps > 0 => projection.step = projection.step.next(),
            Row::Step => projection.step = projection.step.next().next(),
            Row::Horizon => {
                projection.horizon =
                    clamped(i64::from(projection.horizon).saturating_add(by), 1, i64::from(MAX_PERIODS));
            }
            Row::Users => {
                let users = i64::try_from(projection.users).unwrap_or(i64::MAX).saturating_add(by);
                projection.users = clamped(users, 0, MOST_USERS);
            }
            Row::Growth => projection.growth = clamped(i64::from(projection.growth).saturating_add(by), -9_900, 10_000),
            Row::Spend => {
                let spend = i64::try_from(projection.spend).unwrap_or(i64::MAX).saturating_add(by);
                projection.spend = clamped(spend, 0, MOST_SPEND);
            }
            Row::Paying => share(&mut projection.paying),
            Row::Mode => projection.mode = projection.mode.next(),
            Row::Blended => rate(&mut projection.blended),
            Row::Base => rate(&mut projection.base),
            Row::Scale => projection.scale = moved(projection.scale, by, u128::from(MAX_SCALE)),
            Row::DeployerReach => share(&mut projection.deployer_reach),
            Row::BuilderFee => rate(&mut projection.builder_fee),
            Row::BuilderReach => share(&mut projection.builder_reach),
            Row::ReferralShare => share(&mut projection.referral_share),
            Row::ReferralReach => share(&mut projection.referral_reach),
        }
    }

    /// Takes a typed figure in the row's unit, held to what it can mean. False for a figure that
    /// does not read, and for a choice.
    pub fn take(self, projection: &mut Projection, typed: &str) -> bool {
        let Some(value) = self.unit().and_then(|unit| unit.read(typed)) else {
            return false;
        };
        let capped = |ceiling: u128| u32::try_from(value.min(ceiling)).unwrap_or(0);
        match self {
            Row::Start => projection.start = i32::try_from(value.min(24)).unwrap_or(0),
            Row::Horizon => projection.horizon = capped(u128::from(MAX_PERIODS)).max(1),
            Row::Users => projection.users = u64::try_from(value.min(MOST_USERS.unsigned_abs().into())).unwrap_or(0),
            Row::Growth => projection.growth = i32::try_from(value.min(10_000)).unwrap_or(0),
            Row::Spend => projection.spend = value.min(MOST_SPEND.unsigned_abs().into()),
            Row::Paying => projection.paying = capped(PER_SHARE),
            Row::Blended => projection.blended = capped(PER_RATE),
            Row::Base => projection.base = capped(PER_RATE),
            Row::Scale => projection.scale = capped(u128::from(MAX_SCALE)),
            Row::DeployerReach => projection.deployer_reach = capped(PER_SHARE),
            Row::BuilderFee => projection.builder_fee = capped(PER_RATE),
            Row::BuilderReach => projection.builder_reach = capped(PER_SHARE),
            Row::ReferralShare => projection.referral_share = capped(PER_SHARE),
            Row::ReferralReach => projection.referral_reach = capped(PER_SHARE),
            Row::Step | Row::Mode => return false,
        }
        true
    }
}

/// The most traders a projection starts from.
const MOST_USERS: i64 = 100_000_000;

/// The most one trader trades in a period, in cents: a billion dollars.
const MOST_SPEND: i64 = 100_000_000_000;

/// `value` moved `by`, held between zero and `ceiling`.
fn moved(value: u32, by: i64, ceiling: u128) -> u32 {
    let ceiling = i64::try_from(ceiling).unwrap_or(i64::MAX);
    clamped(i64::from(value).saturating_add(by), 0, ceiling)
}

/// `value` held between `low` and `high`, in the type asked for.
fn clamped<T: TryFrom<i64> + Default>(value: i64, low: i64, high: i64) -> T {
    T::try_from(value.clamp(low, high)).unwrap_or_default()
}

/// A plain decimal as a count of `10^-places`, digits past them cut. A sign but `+`, an
/// exponent or a second point is refused.
fn fixed(text: &str, places: usize) -> Option<u128> {
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }
    let body = raw.strip_prefix('+').unwrap_or(raw);
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let whole = if whole.is_empty() { "0" } else { whole };
    if !whole.bytes().all(|byte| byte.is_ascii_digit()) || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut total: u128 = whole.parse().ok()?;
    let digits = fraction.as_bytes();
    for place in 0..places {
        let digit = digits.get(place).map_or(0, |byte| u128::from(byte - b'0'));
        total = total.checked_mul(10)?.checked_add(digit)?;
    }
    Some(total)
}
