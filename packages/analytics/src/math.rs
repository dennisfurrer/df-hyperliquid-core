//! Sums and shares the boards and estimates share: exact `Dec` additions and a part of a whole
//! in hundredths of a percent.

use hl_core::{Dec, Round};

/// `part` of `whole` in hundredths of a percent, cut down; nothing of nothing.
pub fn share(part: Dec, whole: Dec) -> u32 {
    if !whole.is_positive() {
        return 0;
    }
    part.checked_div(whole, 4, Round::Down)
        .and_then(|fraction| fraction.units_at(4, Round::Down))
        .map_or(0, |units| u32::try_from(units.max(0)).unwrap_or(u32::MAX))
}

/// `sum` and `more`, exactly, while a `Dec` holds the total, which no figure here nears; past
/// that, `sum` as it was.
pub fn add(sum: Dec, more: Dec) -> Dec {
    sum.checked_add(more).unwrap_or(sum)
}

/// Every figure added.
pub fn sum(figures: &[Dec]) -> Dec {
    figures.iter().fold(Dec::ZERO, |total, one| add(total, *one))
}
