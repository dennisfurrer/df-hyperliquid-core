//! The touch, from whichever feed is chosen: `bbo` (the best bid and ask, sent when either changes)
//! or the fast `l2Book`. Both send the whole touch every time, so after a reconnect the next frame
//! is already a full snapshot and nothing has to be rebuilt.

use df_hyperliquid_core::Dec;
use df_hyperliquid_core::live::{Frame, Subscription};

/// Which feed the quotes follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `bbo`: the best bid and ask only.
    Bbo,
    /// `l2Book` with `fast: true`.
    FastBook,
}

impl Source {
    /// From `HL_BOOK`: `bbo` (the default) or `fast`.
    pub fn parse(text: &str) -> Option<Source> {
        match text {
            "bbo" => Some(Source::Bbo),
            "fast" => Some(Source::FastBook),
            _ => None,
        }
    }

    /// The subscription for `coin`.
    pub fn subscription(self, coin: &str) -> Subscription {
        match self {
            Source::Bbo => Subscription::Bbo { coin: coin.to_owned() },
            Source::FastBook => {
                Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: true }
            }
        }
    }
}

/// The best bid and ask of `coin` in `frame`; `None` when the frame is about something else or
/// one side of the book is empty.
pub fn touch(frame: &Frame, coin: &str) -> Option<(Dec, Dec)> {
    match frame {
        Frame::Bbo(bbo) if bbo.coin == coin => Some((bbo.bid.as_ref()?.px, bbo.ask.as_ref()?.px)),
        Frame::Book(book) if book.coin == coin => match book.touch() {
            (Some(bid), Some(ask)) => Some((bid, ask)),
            _ => None,
        },
        _ => None,
    }
}
