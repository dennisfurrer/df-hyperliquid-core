//! Hyperliquid's own record of the fills that named a builder: one file a builder a UTC day, at
//! `stats-data.hyperliquid.xyz/Mainnet/builder_fills/{builder}/{YYYYMMDD}.csv.lz4`, as the
//! builder codes page documents. Not Hydromancer's: it is the venue's record, public and keyless,
//! which the dashboard's figures can be checked against. It is published for mainnet only, per
//! builder and never as a ranking. A builder with no fills that day has no file, and the bucket
//! answers 403.
//!
//! The file is an LZ4 frame around a CSV whose header names its columns:
//! `time,user,coin,side,px,sz,crossed,special_trade_type,tif,is_trigger,counterparty,closed_pnl,
//! twap_id,builder_fee`.

use std::collections::{BTreeMap, BTreeSet};

use hl_core::{Address, Dec, Http, Request, Response};

use crate::client::Error;

pub const BUILDER_FILLS: &str = "https://stats-data.hyperliquid.xyz/Mainnet/builder_fills";

/// How an error names this source.
const NAME: &str = "builder_fills";

/// Past this the file is not a day of one builder's fills, and is refused rather than held.
const MOST_BYTES: usize = 256 * 1024 * 1024;

/// Upstream text quoted in an error, at most, in characters.
const QUOTED: usize = 300;

/// Volume, builder fees and fills, summed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// `px * sz` over every fill, in the quote.
    pub volume: Dec,
    /// What the fills paid the builder, in the quote.
    pub fees: Dec,
    pub fills: u64,
}

impl Tally {
    fn take(&mut self, notional: Dec, fee: Dec) {
        self.volume = self.volume.checked_add(notional).unwrap_or(self.volume);
        self.fees = self.fees.checked_add(fee).unwrap_or(self.fees);
        self.fills = self.fills.saturating_add(1);
    }

    #[must_use]
    pub fn plus(self, other: Tally) -> Tally {
        Tally {
            volume: self.volume.checked_add(other.volume).unwrap_or(self.volume),
            fees: self.fees.checked_add(other.fees).unwrap_or(self.fees),
            fills: self.fills.saturating_add(other.fills),
        }
    }
}

/// Which kind of market a coin in the file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind<'a> {
    /// A perp on the native dex.
    Native,
    /// A perp on a HIP-3 dex, named as `perpDexs` names it.
    Hip3(&'a str),
    /// `@index` or `BASE/QUOTE`.
    Spot,
    /// `#` and an outcome coin id.
    Outcome,
}

#[must_use]
pub fn kind_of(coin: &str) -> Kind<'_> {
    if coin.starts_with('#') {
        return Kind::Outcome;
    }
    if coin.starts_with('@') || coin.contains('/') {
        return Kind::Spot;
    }
    match coin.split_once(':') {
        Some((dex, _)) => Kind::Hip3(dex),
        None => Kind::Native,
    }
}

/// One builder's day, as Hyperliquid recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderDay {
    pub builder: Address,
    /// `YYYYMMDD`, as the file is named.
    pub date: String,
    /// Distinct accounts with a fill.
    pub users: u64,
    pub native: Tally,
    pub hip3: Tally,
    pub spot: Tally,
    pub outcomes: Tally,
    /// Each HIP-3 dex the builder routed to, the busiest first.
    pub dexes: Vec<(String, Tally)>,
    /// Each perp, native and HIP-3, the busiest first.
    pub coins: Vec<(String, Tally)>,
}

impl BuilderDay {
    /// Perps on every dex, native and HIP-3.
    #[must_use]
    pub fn perps(&self) -> Tally {
        self.native.plus(self.hip3)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a day: eight digits, YYYYMMDD")]
pub struct BadDate(pub String);

/// The file of `builder`'s fills on `date` (`YYYYMMDD`). It goes to Hyperliquid's bucket with no
/// header at all: no Hydromancer key ever leaves for it.
pub fn request(builder: Address, date: &str) -> Result<Request, BadDate> {
    if date.len() != 8 || !date.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BadDate(date.to_owned()));
    }
    Ok(Request::get(format!("{BUILDER_FILLS}/{}/{date}.csv.lz4", builder.to_hex())))
}

/// The day from the bucket's answer: `None` where it has no file (no fills that day, or not
/// published yet).
pub fn read(builder: Address, date: &str, response: &Response) -> Result<Option<BuilderDay>, Error> {
    if matches!(response.status, 403 | 404) {
        return Ok(None);
    }
    if !response.is_success() {
        let said = response.text().chars().take(QUOTED).collect();
        return Err(Error::Status { name: NAME.to_owned(), status: response.status, said });
    }
    let unreadable = |detail: String| Error::Unreadable { name: NAME.to_owned(), detail };
    let csv = unframe(&response.body).map_err(unreadable)?;
    read_day(builder, date, &String::from_utf8_lossy(&csv)).map(Some).map_err(unreadable)
}

/// One builder's fills on one UTC day, `YYYYMMDD`, or `None` where it has no file.
pub async fn builder_day(http: &impl Http, builder: Address, date: &str) -> Result<Option<BuilderDay>, Error> {
    let request = request(builder, date)
        .map_err(|error| Error::Unreadable { name: NAME.to_owned(), detail: error.to_string() })?;
    let response = http.send(request).await.map_err(|error| Error::Unreachable(error.to_string()))?;
    read(builder, date, &response)
}

/// Sums a day's CSV by market. A line that does not read is skipped; a file without the columns
/// summed is refused.
pub fn read_day(builder: Address, date: &str, csv: &str) -> Result<BuilderDay, String> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().ok_or_else(|| "an empty file".to_owned())?.split(',').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|one| one.trim() == name)
            .ok_or_else(|| format!("no {name} column in {}", header.join(",")))
    };
    let (user, coin, px, sz, fee) =
        (column("user")?, column("coin")?, column("px")?, column("sz")?, column("builder_fee")?);
    let mut day = BuilderDay {
        builder,
        date: date.to_owned(),
        users: 0,
        native: Tally::default(),
        hip3: Tally::default(),
        spot: Tally::default(),
        outcomes: Tally::default(),
        dexes: Vec::new(),
        coins: Vec::new(),
    };
    let mut users = BTreeSet::new();
    let mut dexes: BTreeMap<String, Tally> = BTreeMap::new();
    let mut coins: BTreeMap<String, Tally> = BTreeMap::new();
    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        let (Some(who), Some(market), Some(price), Some(size)) =
            (fields.get(user), fields.get(coin), fields.get(px), fields.get(sz))
        else {
            continue;
        };
        let (Ok(price), Ok(size)) = (Dec::parse(price), Dec::parse(size)) else { continue };
        let Some(notional) = price.checked_mul(size).map(Dec::abs) else { continue };
        let paid = fields.get(fee).and_then(|said| Dec::parse(said).ok()).unwrap_or(Dec::ZERO);
        users.insert(*who);
        match kind_of(market) {
            Kind::Native => {
                day.native.take(notional, paid);
                coins.entry((*market).to_owned()).or_default().take(notional, paid);
            }
            Kind::Hip3(dex) => {
                day.hip3.take(notional, paid);
                dexes.entry(dex.to_owned()).or_default().take(notional, paid);
                coins.entry((*market).to_owned()).or_default().take(notional, paid);
            }
            Kind::Spot => day.spot.take(notional, paid),
            Kind::Outcome => day.outcomes.take(notional, paid),
        }
    }
    day.users = u64::try_from(users.len()).unwrap_or(u64::MAX);
    day.dexes = busiest(dexes);
    day.coins = busiest(coins);
    Ok(day)
}

fn busiest(tallies: BTreeMap<String, Tally>) -> Vec<(String, Tally)> {
    let mut sorted: Vec<(String, Tally)> = tallies.into_iter().collect();
    sorted.sort_by(|a, b| b.1.volume.cmp(&a.1.volume).then_with(|| a.0.cmp(&b.0)));
    sorted
}

/// The contents of an LZ4 frame, or of several one after another. Skippable frames are stepped
/// over; a frame with a dictionary is refused, and so is one cut short anywhere, its checksums
/// included.
pub fn unframe(bytes: &[u8]) -> Result<Vec<u8>, String> {
    const MAGIC: u32 = 0x184D_2204;
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let magic = word(bytes, at)?;
        if magic & 0xFFFF_FFF0 == 0x184D_2A50 {
            let skip = usize::try_from(word(bytes, at + 4)?).map_err(|_| "a skippable frame too long".to_owned())?;
            at = past(bytes, at, 8usize.saturating_add(skip), "a skippable frame")?;
            continue;
        }
        if magic != MAGIC {
            return Err(format!("not an LZ4 frame: magic {magic:#x}"));
        }
        at += 4;
        let flags = *bytes.get(at).ok_or("a frame with no header")?;
        if flags >> 6 != 1 {
            return Err(format!("LZ4 frame version {}", flags >> 6));
        }
        let block_sums = flags & 0x10 != 0;
        let sized = flags & 0x08 != 0;
        let summed = flags & 0x04 != 0;
        if flags & 0x01 != 0 {
            return Err("an LZ4 frame that needs a dictionary".to_owned());
        }
        // Flags, block descriptor, the content size where given, the header checksum.
        at = past(bytes, at, 2 + if sized { 8 } else { 0 } + 1, "the frame header")?;
        loop {
            let size = word(bytes, at)?;
            at += 4;
            if size == 0 {
                break;
            }
            let length = usize::try_from(size & 0x7FFF_FFFF).map_err(|_| "a block too long".to_owned())?;
            let end = past(bytes, at, length, "a block")?;
            let block = bytes.get(at..end).ok_or("a block runs past the end of the file")?;
            match size & 0x8000_0000 != 0 {
                true => out.extend_from_slice(block),
                false => unblock(block, &mut out)?,
            }
            if out.len() > MOST_BYTES {
                return Err("more than a day of fills".to_owned());
            }
            at = past(bytes, end, if block_sums { 4 } else { 0 }, "a block checksum")?;
        }
        at = past(bytes, at, if summed { 4 } else { 0 }, "the content checksum")?;
    }
    Ok(out)
}

/// `at + length`, where those bytes are all there.
fn past(bytes: &[u8], at: usize, length: usize, what: &str) -> Result<usize, String> {
    at.checked_add(length)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| format!("{what} runs past the end of the file"))
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    let end = at.checked_add(4).ok_or("the frame ends inside a header")?;
    let four: [u8; 4] =
        bytes.get(at..end).and_then(|four| four.try_into().ok()).ok_or("the frame ends inside a header")?;
    Ok(u32::from_le_bytes(four))
}

/// One compressed block, appended to what came before it: a linked block may copy from the
/// blocks ahead of it.
fn unblock(block: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    let mut at = 0usize;
    let length = |at: &mut usize, start: usize| -> Result<usize, String> {
        let mut total = start;
        if start == 15 {
            loop {
                let more = *block.get(*at).ok_or("a length runs off the block")?;
                *at += 1;
                total = total.checked_add(usize::from(more)).ok_or("a length past what fits")?;
                if more != 255 {
                    break;
                }
            }
        }
        Ok(total)
    };
    while let Some(&token) = block.get(at) {
        at += 1;
        let literals = length(&mut at, usize::from(token >> 4))?;
        let end = at.checked_add(literals).ok_or("literals run off the block")?;
        out.extend_from_slice(block.get(at..end).ok_or("literals run off the block")?);
        at = end;
        if at >= block.len() {
            break;
        }
        let (Some(&low), Some(&high)) = (block.get(at), block.get(at + 1)) else {
            return Err("an offset runs off the block".to_owned());
        };
        at += 2;
        let back = usize::from(u16::from_le_bytes([low, high]));
        if back == 0 || back > out.len() {
            return Err(format!("an offset of {back} with {} bytes out", out.len()));
        }
        let matched = length(&mut at, usize::from(token & 0x0F))?.checked_add(4).ok_or("a match past what fits")?;
        if out.len().saturating_add(matched) > MOST_BYTES {
            return Err("more than a day of fills".to_owned());
        }
        let from = out.len() - back;
        for step in 0..matched {
            let byte = *out.get(from + step).ok_or("a match reads past what was written")?;
            out.push(byte);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/days.rs"]
mod tests;
