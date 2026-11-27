//! The position and the profit and loss, kept from the account's own fills: each fill moves the
//! position by its size and the cash by its value and fee, so the profit at any mark is the cash
//! plus the position valued at that mark.

use df_hyperliquid_core::Dec;
use df_hyperliquid_core::markets::Side;

/// What the fills since the start add up to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Inventory {
    /// The position, in the base asset: positive long, negative short.
    pub position: Dec,
    /// Cash from the fills, fees taken off: what the position cost, negated.
    pub cash: Dec,
    /// Fees paid; negative when rebates exceed them.
    pub fees: Dec,
    /// How many fills.
    pub fills: u64,
}

impl Inventory {
    /// Takes one fill: `side` as the account traded, at `px` for `sz`, paying `fee`. `None` when
    /// the arithmetic leaves the decimal range.
    pub fn fill(&mut self, side: Side, px: Dec, sz: Dec, fee: Dec) -> Option<()> {
        let value = px.checked_mul(sz)?;
        match side {
            Side::Buy => {
                self.position = self.position.checked_add(sz)?;
                self.cash = self.cash.checked_sub(value)?;
            }
            Side::Sell => {
                self.position = self.position.checked_sub(sz)?;
                self.cash = self.cash.checked_add(value)?;
            }
        }
        self.cash = self.cash.checked_sub(fee)?;
        self.fees = self.fees.checked_add(fee)?;
        self.fills += 1;
        Some(())
    }

    /// The profit and loss with the position valued at `mark`.
    pub fn pnl(&self, mark: Dec) -> Option<Dec> {
        self.cash.checked_add(self.position.checked_mul(mark)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Dec {
        text.parse().expect("a decimal")
    }

    #[test]
    fn a_round_trip_keeps_the_spread_less_fees() {
        let mut inventory = Inventory::default();
        inventory.fill(Side::Buy, dec("91000"), dec("0.001"), dec("0.01")).expect("buy");
        assert_eq!(inventory.position, dec("0.001"));
        inventory.fill(Side::Sell, dec("91010"), dec("0.001"), dec("0.01")).expect("sell");
        assert_eq!(inventory.position, Dec::ZERO);
        // 0.001 x 10 = 0.01 earned, 0.02 paid in fees.
        assert_eq!(inventory.pnl(dec("95000")), Some(dec("-0.01")));
        assert_eq!(inventory.fills, 2);
    }

    #[test]
    fn an_open_position_is_valued_at_the_mark() {
        let mut inventory = Inventory::default();
        inventory.fill(Side::Sell, dec("91000"), dec("0.002"), Dec::ZERO).expect("sell");
        assert_eq!(inventory.pnl(dec("90000")), Some(dec("2")));
        assert_eq!(inventory.pnl(dec("92000")), Some(dec("-2")));
    }
}
