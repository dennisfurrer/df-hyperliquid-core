//! The catalogue: one `outcomeMeta` read into markets and questions, each with its family, a title
//! in words, its sides and its deadline. A question holds named outcomes, exactly one of which
//! settles at 1, and a fallback; its legs take its deadline and title when they have none.

use std::collections::BTreeMap;

use crate::markets::outcomes::coin::Coin;
use crate::markets::outcomes::fields::Fields;
use crate::markets::outcomes::meta::{Outcome as RawOutcome, OutcomeMeta, Question as RawQuestion};
use crate::markets::outcomes::template::{Family, Pattern, Patterns, Template};
use crate::markets::outcomes::time::Stamp;
use crate::utils::{Dec, OutcomeSide};

mod interpret;

pub use interpret::{Details, Pair};
use interpret::{bucket_titles, from_pattern, interpret, label};

/// Whether a market's deadline has passed. An expired market still trades until it settles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Open,
    Expired,
}

impl Status {
    /// Expired from the deadline's own minute on.
    #[must_use]
    pub fn at(expiry: Option<Stamp>, now_ms: u64) -> Status {
        match expiry {
            Some(deadline) if deadline.passed(now_ms) => Status::Expired,
            _ => Status::Open,
        }
    }
}

/// A market's two coins and what each is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sides {
    pub yes: Coin,
    pub no: Coin,
    /// `Yes`, or a template's side name filled in: `Eagles`.
    pub yes_label: String,
    pub no_label: String,
}

/// One outcome, read from `outcomeMeta`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Market {
    /// The first side's coin, which names the market.
    pub market: Coin,
    pub outcome: u32,
    pub kind: Family,
    /// What the market asks, in words.
    pub title: String,
    /// The outcome's name as listed: `template:binaryPrice4`.
    pub template: String,
    pub template_version: Option<u8>,
    /// As listed, which a settlement repeats character for character.
    pub description: String,
    pub status: Status,
    pub expiry: Option<Stamp>,
    /// The deployer's venue; the venue's own daily markets have none.
    pub venue: Option<String>,
    pub quote: Option<String>,
    /// Read back as `1.0`.
    pub fee_scale: Option<Dec>,
    /// The question it is a leg of, named or fallback.
    pub question: Option<u32>,
    pub question_title: Option<String>,
    pub sides: Sides,
    pub details: Details,
}

impl Market {
    /// The title, with its question's before it when the two differ.
    #[must_use]
    pub fn headline(&self) -> String {
        match &self.question_title {
            Some(question) if question != &self.title => format!("{question} / {}", self.title),
            _ => self.title.clone(),
        }
    }

    /// Whether `needle` is in its title, description, family, coin, venue or question, ignoring
    /// ASCII case.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        contains(&self.title, needle)
            || contains(&self.description, needle)
            || contains(self.kind.slug(), needle)
            || self.market.matches(needle)
            || self.venue.as_deref().is_some_and(|venue| contains(venue, needle))
            || self.question_title.as_deref().is_some_and(|question| contains(question, needle))
    }
}

/// One question, read: its named legs still live, how many have settled, and its fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub question: u32,
    pub kind: Family,
    pub title: String,
    pub template: String,
    pub description: String,
    pub status: Status,
    pub expiry: Option<Stamp>,
    /// Its legs' venue.
    pub venue: Option<String>,
    /// Each named leg `outcomeMeta` still lists, by its first side.
    pub legs: Vec<Coin>,
    pub settled_legs: usize,
    pub fallback: Option<Coin>,
    pub details: Details,
}

/// Whether `needle` appears in `haystack`, ignoring ASCII case.
#[must_use]
pub fn contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let (hay, needle) = (haystack.as_bytes(), needle.as_bytes());
    needle.len() <= hay.len()
        && hay.windows(needle.len()).any(|window| window.iter().zip(needle).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

/// Everything one `outcomeMeta` says, read: markets soonest to expire first, then by family and
/// coin; questions soonest first, then by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub markets: Vec<Market>,
    pub questions: Vec<Question>,
    by_outcome: BTreeMap<u32, usize>,
    by_question: BTreeMap<u32, usize>,
}

/// What a leg takes from its question.
struct Owner {
    id: u32,
    title: String,
    expiry: Option<Stamp>,
    /// Titles for legs the venue names only by `index:N`.
    leg_titles: Vec<String>,
}

impl Catalog {
    #[must_use]
    pub fn build(meta: &OutcomeMeta, now_ms: u64) -> Catalog {
        Catalog::build_with(meta, &Patterns::new(), now_ms)
    }

    /// As [`Catalog::build`], with the venue's templates to read any template that has no
    /// [`Family`] here.
    #[must_use]
    pub fn build_with(meta: &OutcomeMeta, patterns: &Patterns, now_ms: u64) -> Catalog {
        let by_id: BTreeMap<u32, &RawOutcome> = meta.outcomes.iter().map(|raw| (raw.outcome, raw)).collect();
        let mut questions = Vec::with_capacity(meta.questions.len());
        let mut owners: BTreeMap<u32, Owner> = BTreeMap::new();
        for raw in &meta.questions {
            let question = build_question(raw, &by_id, patterns, now_ms);
            let leg_titles = bucket_titles(&Fields::parse(&raw.description));
            for leg in raw.named_outcomes.iter().chain(raw.fallback_outcome.iter()) {
                owners.insert(
                    *leg,
                    Owner {
                        id: raw.question,
                        title: question.title.clone(),
                        expiry: question.expiry,
                        leg_titles: leg_titles.clone(),
                    },
                );
            }
            questions.push(question);
        }
        let mut markets: Vec<Market> = meta
            .outcomes
            .iter()
            .filter(|raw| raw.side_specs.len() == 2)
            .map(|raw| build_market(raw, owners.get(&raw.outcome), patterns, now_ms))
            .collect();
        markets.sort_by_key(|market| (market.expiry.is_none(), market.expiry, market.kind, market.market));
        questions.sort_by_key(|question| (question.expiry.is_none(), question.expiry, question.question));
        Catalog::of(markets, questions)
    }

    /// A catalogue of what is given, as given.
    #[must_use]
    pub fn of(markets: Vec<Market>, questions: Vec<Question>) -> Catalog {
        let mut catalog = Catalog { markets, questions, ..Catalog::default() };
        catalog.reindex();
        catalog
    }

    fn reindex(&mut self) {
        self.by_outcome = self.markets.iter().enumerate().map(|(at, market)| (market.outcome, at)).collect();
        self.by_question = self.questions.iter().enumerate().map(|(at, question)| (question.question, at)).collect();
    }

    /// Only what `venue` deployed.
    pub fn retain_venue(&mut self, venue: &str) {
        self.markets.retain(|market| market.venue.as_deref() == Some(venue));
        self.questions.retain(|question| question.venue.as_deref() == Some(venue));
        self.reindex();
    }

    /// Only these families.
    pub fn retain_kinds(&mut self, kinds: &[Family]) {
        self.markets.retain(|market| kinds.contains(&market.kind));
        self.questions.retain(|question| kinds.contains(&question.kind));
        self.reindex();
    }

    /// The market either side of `coin` belongs to.
    #[must_use]
    pub fn find(&self, coin: Coin) -> Option<&Market> {
        self.markets.get(self.seat(coin)?)
    }

    /// Where that market sits in [`Catalog::markets`].
    #[must_use]
    pub fn seat(&self, coin: Coin) -> Option<usize> {
        self.by_outcome.get(&coin.outcome()).copied()
    }

    /// Where a question sits in [`Catalog::questions`].
    #[must_use]
    pub fn place_of(&self, question: u32) -> Option<usize> {
        self.by_question.get(&question).copied()
    }

    #[must_use]
    pub fn question(&self, question: u32) -> Option<&Question> {
        self.questions.get(self.place_of(question)?)
    }
}

/// What a template means for one market or question: from the venue's pattern when it has no
/// family here, from its family otherwise.
fn read(
    family: Family,
    patterns: &Patterns,
    fields: &Fields<'_>,
    description: &str,
    name: &str,
) -> (Option<Stamp>, Details, String) {
    match family {
        Family::Template => {
            let id = name.strip_prefix("template:").unwrap_or(name);
            let pattern: Option<&Pattern> = patterns.get(id);
            from_pattern(pattern, fields, id)
        }
        family => interpret(family, fields, description, name),
    }
}

fn build_market(raw: &RawOutcome, owner: Option<&Owner>, patterns: &Patterns, now_ms: u64) -> Market {
    let template = Template::classify(&raw.name);
    let fields = Fields::parse(&raw.description);
    let (own_expiry, details, title) = read(template.family, patterns, &fields, &raw.description, &raw.name);
    let expiry = own_expiry.or_else(|| owner.and_then(|owner| owner.expiry));
    let title = fields
        .seconds("index")
        .and_then(|index| owner?.leg_titles.get(usize::try_from(index).ok()?).cloned())
        .unwrap_or(title);
    let side = |at: usize| raw.side_specs.get(at).map(|side| side.name.as_str());
    Market {
        market: Coin::new(raw.outcome, OutcomeSide::Yes),
        outcome: raw.outcome,
        kind: template.family,
        title,
        template: raw.name.clone(),
        template_version: template.version,
        description: raw.description.clone(),
        status: Status::at(expiry, now_ms),
        expiry,
        venue: raw.venue.clone(),
        quote: raw.quote_token.clone(),
        fee_scale: raw.deployer_fee_scale,
        question: owner.map(|owner| owner.id),
        question_title: owner.map(|owner| owner.title.clone()),
        sides: Sides {
            yes: Coin::new(raw.outcome, OutcomeSide::Yes),
            no: Coin::new(raw.outcome, OutcomeSide::No),
            yes_label: label(side(0), &fields, "Yes"),
            no_label: label(side(1), &fields, "No"),
        },
        details,
    }
}

fn build_question(raw: &RawQuestion, by_id: &BTreeMap<u32, &RawOutcome>, patterns: &Patterns, now_ms: u64) -> Question {
    let template = Template::classify(&raw.name);
    let fields = Fields::parse(&raw.description);
    let (expiry, details, title) = read(template.family, patterns, &fields, &raw.description, &raw.name);
    let venue = raw
        .named_outcomes
        .iter()
        .chain(raw.fallback_outcome.iter())
        .find_map(|id| by_id.get(id).and_then(|outcome| outcome.venue.clone()));
    let listed = |id: &u32| by_id.contains_key(id);
    Question {
        question: raw.question,
        kind: template.family,
        title,
        template: raw.name.clone(),
        description: raw.description.clone(),
        status: Status::at(expiry, now_ms),
        expiry,
        venue,
        legs: raw.named_outcomes.iter().filter(|id| listed(id)).map(|id| Coin::new(*id, OutcomeSide::Yes)).collect(),
        settled_legs: raw.settled_named_outcomes.len(),
        fallback: raw.fallback_outcome.filter(listed).map(|id| Coin::new(id, OutcomeSide::Yes)),
        details,
    }
}

#[cfg(test)]
#[path = "tests/market.rs"]
mod tests;
