//! Where the time goes, per requote: reading the frame, working out the quotes, signing, and the
//! whole from the frame's arrival to a signed request ready to send; and, when orders are sent,
//! from sending to the venue's answer. Each is kept as samples and shown as a median and a p99.

use std::time::Duration;

/// The most samples kept per figure; past it the oldest half is dropped.
const KEEP: usize = 200_000;

/// The samples of one figure, in nanoseconds.
#[derive(Debug, Default)]
pub struct Samples(Vec<u64>);

impl Samples {
    /// Adds one sample.
    pub fn add(&mut self, took: Duration) {
        if self.0.len() >= KEEP {
            self.0.drain(..KEEP / 2);
        }
        self.0.push(u64::try_from(took.as_nanos()).unwrap_or(u64::MAX));
    }

    /// How many samples.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The median and the p99, in nanoseconds; `None` with no samples.
    pub fn median_p99(&self) -> Option<(u64, u64)> {
        if self.0.is_empty() {
            return None;
        }
        let mut sorted = self.0.clone();
        sorted.sort_unstable();
        // Nearest rank: the smallest sample with at least `percent` of them at or under it.
        let at = |percent: usize| {
            let rank = (sorted.len() * percent).div_ceil(100);
            sorted[rank.clamp(1, sorted.len()) - 1]
        };
        Some((at(50), at(99)))
    }
}

/// Every figure the market maker reports.
#[derive(Debug, Default)]
pub struct Stats {
    /// Reading the frame's text into the SDK's types.
    pub parse: Samples,
    /// Working out the quotes and what changes.
    pub quote: Samples,
    /// Building, hashing, signing and writing the request body.
    pub sign: Samples,
    /// From the frame's arrival to the signed request ready to send.
    pub total: Samples,
    /// From sending a request to the venue's answer (only when sending).
    pub ack: Samples,
    /// Book frames read.
    pub frames: u64,
    /// Requotes signed.
    pub requotes: u64,
    /// Book frames that changed no quote by a tick or more.
    pub unchanged: u64,
    /// Requotes held back to save the request budget.
    pub paced: u64,
    /// Post-only quotes the venue refused because they would have crossed: normal in a fast
    /// market.
    pub would_cross: u64,
    /// Modifies of a quote that filled or was cancelled after the modify was sent: normal when
    /// quotes fill.
    pub already_gone: u64,
    /// Requests not sent because the websocket was reconnecting.
    pub unsent: u64,
    /// Other refusals.
    pub refused: u64,
}

impl Stats {
    /// The report, one figure a line.
    pub fn report(&self, elapsed: Duration, sending: bool) -> String {
        let micros = |samples: &Samples| match samples.median_p99() {
            Some((median, p99)) => format!(
                "median {:>9} us   p99 {:>9} us   ({} samples)",
                tenths(median, 1_000),
                tenths(p99, 1_000),
                samples.len()
            ),
            None => "no samples".to_owned(),
        };
        let mut lines = vec![
            format!(
                "after {} s: {} book frames, {} requotes signed, {} unchanged, {} held back by the gap between requotes",
                elapsed.as_secs(),
                self.frames,
                self.requotes,
                self.unchanged,
                self.paced
            ),
            format!("  parse frame        {}", micros(&self.parse)),
            format!("  quote logic        {}", micros(&self.quote)),
            format!("  build and sign     {}", micros(&self.sign)),
            format!("  frame to signed    {}", micros(&self.total)),
        ];
        if sending {
            let millis = match self.ack.median_p99() {
                Some((median, p99)) => format!(
                    "median {:>9} ms   p99 {:>9} ms   ({} answers)",
                    tenths(median, 1_000_000),
                    tenths(p99, 1_000_000),
                    self.ack.len()
                ),
                None => "no answers yet".to_owned(),
            };
            lines.push(format!("  send to answer     {millis}"));
            lines.push(format!(
                "  refused: {} would have crossed (normal), {} modified after a fill or cancel (normal), {} other",
                self.would_cross, self.already_gone, self.refused
            ));
            if self.unsent > 0 {
                lines.push(format!("  not sent while the websocket reconnected: {}", self.unsent));
            }
        }
        lines.join("\n")
    }
}

/// `ns` in units of `unit` nanoseconds, to one decimal place, rounded to the nearest.
fn tenths(ns: u64, unit: u64) -> String {
    let tenths = (ns.saturating_mul(10) + unit / 2) / unit;
    format!("{}.{}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_p99_by_nearest_rank() {
        let mut samples = Samples::default();
        for ns in 1..=100 {
            samples.add(Duration::from_nanos(ns));
        }
        assert_eq!(samples.median_p99(), Some((50, 99)));
        assert_eq!(Samples::default().median_p99(), None);
        assert_eq!(tenths(25_340, 1_000), "25.3");
        assert_eq!(tenths(25_350, 1_000), "25.4");
    }

    #[test]
    fn the_report_counts_normal_refusals_apart_from_the_rest() {
        let stats = Stats { would_cross: 2, already_gone: 3, refused: 1, ..Stats::default() };
        let report = stats.report(Duration::from_secs(60), true);
        assert!(
            report.contains(
                "refused: 2 would have crossed (normal), 3 modified after a fill or cancel (normal), 1 other"
            )
        );
        assert!(!report.contains("not sent"), "nothing about reconnects when every request was sent");
        let reconnected = Stats { unsent: 4, ..Stats::default() }.report(Duration::from_secs(60), true);
        assert!(reconnected.contains("not sent while the websocket reconnected: 4"));
    }
}
