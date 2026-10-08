//! What each family's description says, in words: a title, the deadline, and
//! the fields that belong to that family alone. Numbers are carried as the
//! chain wrote them, `95000.00` included, since they are shown, not computed.

use crate::{
    fields::Fields,
    template::{Family, Pattern, RateMove},
    time::Stamp,
};

/// Two of something: a contest's participants, their short names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub a: String,
    pub b: String,
}

/// The fields that belong to one family and no other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Details {
    /// A perp's price above a threshold, touching a target, or in a range.
    Price {
        asset: String,
        threshold: Option<String>,
        target: Option<String>,
        low: Option<String>,
        high: Option<String>,
        /// The TWAP window it settles on.
        window_seconds: Option<u32>,
        price_description: Option<String>,
    },
    Contest {
        participants: Pair,
        short_names: Option<Pair>,
        starts: Option<Stamp>,
        sport: Option<String>,
        competition: Option<String>,
        stage: Option<String>,
        contest_type: Option<String>,
        counted_play: Option<String>,
        source: Option<String>,
        season: Option<String>,
    },
    Competition {
        competition: Option<String>,
        sport: Option<String>,
        season: Option<String>,
        source: Option<String>,
    },
    /// One entrant standing for itself: a participant, the draw.
    Entrant {
        participant: String,
    },
    Measure {
        measure: String,
        line: Option<String>,
        low: Option<String>,
        high: Option<String>,
        event: Option<String>,
        sport: Option<String>,
        counted_play: Option<String>,
        starts: Option<Stamp>,
        source: Option<String>,
    },
    Rate {
        movement: Option<RateMove>,
        institution: Option<String>,
        measure: Option<String>,
        label: Option<String>,
        decides: Option<Stamp>,
        source: Option<String>,
    },
    /// The venue's daily markets: a price binary, or a price range bucketed
    /// by `thresholds`.
    Recurring {
        class: Option<String>,
        asset: Option<String>,
        target: Option<String>,
        thresholds: Vec<String>,
        period: Option<String>,
    },
    /// A template read from the venue's pattern: its id and each keyword's
    /// value, a time written as a [`Stamp`] reads.
    Template {
        template: String,
        fields: Vec<(String, String)>,
    },
    /// Nothing structured on chain: the description verbatim.
    Prose {
        text: String,
    },
}

/// The keywords that name a template's deadline, the first found winning.
const DEADLINES: [&str; 5] = ["resolutionDeadline", "decisionDeadline", "expiry", "time", "dateTime"];

/// A market or question on a template this build has no family for, read
/// from the venue's pattern for it: the title written as the pattern says,
/// the deadline from its first deadline keyword (or else its last time that
/// is not a scheduled start), and every keyword as a field. Without the
/// pattern the fields come in the description's order and the title is their
/// values.
pub(super) fn from_pattern(
    pattern: Option<&Pattern>,
    fields: &Fields<'_>,
    id: &str,
) -> (Option<Stamp>, Details, String) {
    let keywords: Vec<(String, bool)> = match pattern {
        // What the title is written from leads, in its order, so what sets
        // this market apart comes first; the rest in the venue's order.
        Some(pattern) => {
            let named = braced(&pattern.name);
            let mut keywords: Vec<(String, bool)> =
                pattern.keywords.iter().map(|(name, kind)| (name.clone(), kind == "dateTime")).collect();
            keywords.sort_by_key(|(name, _)| named.iter().position(|one| one == name).unwrap_or(named.len()));
            keywords
        }
        None => fields.pairs().map(|(name, value)| (name.to_owned(), Stamp::parse(value).is_some())).collect(),
    };
    let value = |name: &str, timed: bool| {
        let text = fields.text(name)?;
        Some(match timed {
            true => Stamp::parse(&text).map_or(text, |at| at.to_string()),
            false => text,
        })
    };
    let written: Vec<(String, String)> =
        keywords.iter().filter_map(|(name, timed)| Some((name.clone(), value(name, *timed)?))).collect();
    let times: Vec<&String> = keywords.iter().filter(|(_, timed)| *timed).map(|(name, _)| name).collect();
    let expiry = DEADLINES
        .iter()
        .filter(|name| times.iter().any(|time| time == *name))
        .find_map(|name| fields.time(name))
        .or_else(|| {
            times.iter().rev().filter(|name| !name.starts_with("scheduled")).find_map(|name| fields.time(name))
        });
    let title = match pattern {
        Some(pattern) => fill(&pattern.name, |name| value(name, times.iter().any(|time| *time == name))),
        None => written.iter().map(|(_, value)| value.as_str()).collect::<Vec<_>>().join(", "),
    };
    let title = match title.trim().is_empty() {
        true => id.to_owned(),
        false => title.trim().to_owned(),
    };
    (expiry, Details::Template { template: id.to_owned(), fields: written }, title)
}

/// The keywords `pattern` names in braces, in its order.
fn braced(pattern: &str) -> Vec<String> {
    pattern.split('{').skip(1).filter_map(|part| part.split_once('}').map(|(name, _)| name.to_owned())).collect()
}

/// `pattern` with each `{keyword}` replaced by its value, or by `?` when the
/// outcome gives none.
pub(super) fn fill(pattern: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                out.push_str(&value(&after[..close]).unwrap_or_else(|| "?".to_owned()));
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// A side's name: `template:Yes` is `Yes`, `template:{shortNameA}` the
/// description's short name, nothing at all `fallback`.
pub(super) fn label(spec: Option<&str>, fields: &Fields<'_>, fallback: &str) -> String {
    let raw = spec.unwrap_or(fallback).trim();
    let raw = raw.strip_prefix("template:").unwrap_or(raw);
    let Some(key) = raw.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) else {
        return match raw.is_empty() {
            true => fallback.to_owned(),
            false => raw.to_owned(),
        };
    };
    fields.text(key).unwrap_or_else(|| fallback.to_owned())
}

fn pair(fields: &Fields<'_>, a: &str, b: &str) -> Option<Pair> {
    Some(Pair { a: fields.text(a)?, b: fields.text(b)? })
}

fn price_details(fields: &Fields<'_>) -> Details {
    Details::Price {
        asset: fields.text("perp").unwrap_or_else(|| "?".to_owned()),
        threshold: fields.number("threshold"),
        target: fields.number("target"),
        low: fields.number("low"),
        high: fields.number("high"),
        window_seconds: fields.seconds("seconds"),
        price_description: fields.text("priceDescription"),
    }
}

fn contest_details(fields: &Fields<'_>) -> Details {
    Details::Contest {
        participants: pair(fields, "participantA", "participantB")
            .unwrap_or_else(|| Pair { a: "A".to_owned(), b: "B".to_owned() }),
        short_names: pair(fields, "shortNameA", "shortNameB"),
        starts: fields.time("scheduledStart"),
        sport: fields.text("sport"),
        competition: fields.text("competition"),
        stage: fields.text("stage"),
        contest_type: fields.text("contestType"),
        counted_play: fields.text("countedPlay"),
        source: fields.text("officialSource"),
        season: fields.text("season"),
    }
}

/// `A vs B (Competition Stage)`.
fn contest_title(fields: &Fields<'_>) -> String {
    let contest = match pair(fields, "participantA", "participantB") {
        Some(sides) => format!("{} vs {}", sides.a, sides.b),
        None => fields.text("competition").unwrap_or_else(|| "contest".to_owned()),
    };
    match (fields.text("competition"), fields.text("stage")) {
        (Some(competition), Some(stage)) => format!("{contest} ({competition} {stage})"),
        (Some(competition), None) => format!("{contest} ({competition})"),
        _ => contest,
    }
}

fn measure_details(fields: &Fields<'_>) -> Details {
    Details::Measure {
        measure: fields.text("measure").unwrap_or_else(|| "?".to_owned()),
        line: fields.number("line"),
        low: fields.number("low"),
        high: fields.number("high"),
        event: fields.text("event"),
        sport: fields.text("sport"),
        counted_play: fields.text("countedPlay"),
        starts: fields.time("scheduledStart"),
        source: fields.text("officialSource"),
    }
}

/// `points over 45.5 (Steelers v Browns)`.
fn measure_title(fields: &Fields<'_>) -> String {
    let measure = fields.text("measure").unwrap_or_else(|| "measure".to_owned());
    let bound = fields.number("line").or_else(|| fields.number("low"));
    let event = fields.text("event").map(|event| format!(" ({event})")).unwrap_or_default();
    match bound {
        Some(line) => format!("{measure} over {line}{event}"),
        None => format!("{measure}{event}"),
    }
}

const fn rate_word(movement: RateMove) -> &'static str {
    match movement {
        RateMove::Increase => "rate increase",
        RateMove::Decrease => "rate decrease",
        RateMove::NoChange => "no rate change",
    }
}

/// A time as a title writes it, or nothing.
fn when(at: Option<Stamp>) -> String {
    at.map(|at| at.to_string()).unwrap_or_default()
}

/// The deadline, the details and the title of a family this build knows.
pub(super) fn interpret(
    family: Family,
    fields: &Fields<'_>,
    description: &str,
    name: &str,
) -> (Option<Stamp>, Details, String) {
    let perp = || fields.text("perp").unwrap_or_else(|| "?".to_owned());
    let number = |key: &str| fields.number(key).unwrap_or_default();
    match family {
        Family::BinaryPrice => {
            let expiry = fields.time("time");
            let title = format!("{} above {} at {}", perp(), number("threshold"), when(expiry));
            (expiry, price_details(fields), title)
        }
        Family::PriceTouch => {
            let expiry = fields.time("time");
            let title = format!("{} touches {} by {}", perp(), number("target"), when(expiry));
            (expiry, price_details(fields), title)
        }
        Family::ScalarPrice => {
            let expiry = fields.time("time");
            let title = format!("{} in {} to {} at {}", perp(), number("low"), number("high"), when(expiry));
            (expiry, price_details(fields), title)
        }
        Family::ContestWinner | Family::ContestResult => {
            (fields.time("resolutionDeadline"), contest_details(fields), contest_title(fields))
        }
        Family::ContestParticipant | Family::TournamentParticipant => {
            let participant = fields.text("participant").unwrap_or_else(|| name.to_owned());
            (None, Details::Entrant { participant: participant.clone() }, participant)
        }
        Family::ContestDraw => (None, Details::Entrant { participant: "Draw".to_owned() }, "Draw".to_owned()),
        Family::SportsOverUnder | Family::SportsScalar => {
            (fields.time("resolutionDeadline"), measure_details(fields), measure_title(fields))
        }
        Family::PolicyRate(movement) => (
            None,
            Details::Rate {
                movement: Some(movement),
                institution: None,
                measure: None,
                label: None,
                decides: None,
                source: None,
            },
            rate_word(movement).to_owned(),
        ),
        Family::PolicyRateDecision => {
            let label = fields.text("decisionLabel");
            let institution = fields.text("institution");
            let title = match (&institution, &label) {
                (Some(who), Some(when)) => format!("{who}, {when}"),
                (Some(who), None) => who.clone(),
                (None, Some(when)) => format!("rate decision {when}"),
                (None, None) => "rate decision".to_owned(),
            };
            let details = Details::Rate {
                movement: None,
                institution,
                measure: fields.text("policyMeasure"),
                label,
                decides: fields.time("scheduledDecision"),
                source: fields.text("officialSource"),
            };
            (fields.time("decisionDeadline"), details, title)
        }
        Family::TournamentWinner => {
            let competition = fields.text("competition");
            let season = fields.text("season");
            let title = match (&competition, &season) {
                (Some(competition), Some(season)) => format!("{competition} {season} winner"),
                (Some(competition), None) => format!("{competition} winner"),
                _ => "tournament winner".to_owned(),
            };
            let details = Details::Competition {
                competition,
                sport: fields.text("sport"),
                season,
                source: fields.text("officialSource"),
            };
            (fields.time("resolutionDeadline"), details, title)
        }
        Family::Recurring => {
            let expiry = fields.time("expiry");
            let asset = fields.text("underlying");
            let target = fields.number("targetPrice");
            let thresholds = fields.numbers("priceThresholds");
            let title = match (&asset, &target) {
                (Some(asset), Some(target)) => format!("{asset} above {target} at {}", when(expiry)),
                (Some(asset), None) if !thresholds.is_empty() => format!("{asset} price range at {}", when(expiry)),
                _ => name.to_owned(),
            };
            let details = Details::Recurring {
                class: fields.text("class"),
                asset,
                target,
                thresholds,
                period: fields.text("period"),
            };
            (expiry, details, title)
        }
        // Read from the venue's pattern by the caller, which holds it.
        Family::Template => from_pattern(None, fields, name.strip_prefix("template:").unwrap_or(name)),
        Family::Fallback => {
            (None, Details::Prose { text: description.to_owned() }, "none of the listed outcomes".to_owned())
        }
        Family::Named => (Stamp::parse(description), Details::Prose { text: description.to_owned() }, name.to_owned()),
    }
}

/// A price range's legs, by index: below the first threshold, between each
/// two, above the last.
pub(super) fn bucket_titles(fields: &Fields<'_>) -> Vec<String> {
    let thresholds = fields.numbers("priceThresholds");
    let (Some(first), Some(last)) = (thresholds.first(), thresholds.last()) else {
        return Vec::new();
    };
    let asset = fields.text("underlying").unwrap_or_else(|| "price".to_owned());
    let mut titles = vec![format!("{asset} below {first}")];
    titles.extend(thresholds.windows(2).map(|pair| format!("{asset} {} to {}", pair[0], pair[1])));
    titles.push(format!("{asset} above {last}"));
    titles
}
