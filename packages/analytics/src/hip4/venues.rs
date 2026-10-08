//! Who lists what: each venue's markets and questions from the catalogue (hl-hip4's [`Catalog`],
//! which a caller may have narrowed to the venues or kinds it shows), and from `outcomeMeta`
//! itself what the venue's cap counts, every row it holds, and who deploys it. The venue's own
//! daily markets are a row of their own, with no deployer and no cap.

use std::collections::BTreeMap;

use hl_core::Address;
use hl_hip4::{
    Catalog, DeployerLimits, Family, OutcomeMeta, OutcomeTemplate, Remaining, Status, SubDeployer, Template,
};

/// The first market a venue listed: its lowest outcome id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct First {
    pub outcome: u32,
    pub title: String,
}

/// One venue's listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Venue {
    /// `None` for the venue's own daily markets.
    pub venue: Option<String>,
    /// Markets in the catalogue.
    pub markets: usize,
    pub questions: usize,
    /// Markets whose deadline has not passed.
    pub open: usize,
    /// Every `outcomeMeta` row on the venue, fallbacks and expired ones included: what its cap
    /// of live outcomes counts, whatever the catalogue was narrowed to.
    pub live: usize,
    pub deployer: Option<Address>,
    /// Who the deployer lets send which operations, the address granted most first.
    pub subs: Vec<SubDeployer>,
    pub first: Option<First>,
}

impl Venue {
    /// What `outcomeDeployerLimits` said remains, beside the live count it counts down from.
    #[must_use]
    pub fn limits(&self, remaining: Remaining) -> DeployerLimits {
        DeployerLimits::new(remaining, u32::try_from(self.live).unwrap_or(u32::MAX))
    }
}

/// Every venue the catalogue lists, the most markets first.
#[must_use]
pub fn venues(meta: &OutcomeMeta, catalog: &Catalog) -> Vec<Venue> {
    let mut found: BTreeMap<Option<&str>, Venue> = BTreeMap::new();
    let slot = |venue: Option<&str>| -> Venue {
        let deployer = venue.and_then(|venue| meta.deployer(venue));
        Venue {
            venue: venue.map(str::to_owned),
            markets: 0,
            questions: 0,
            open: 0,
            live: venue
                .map_or(0, |venue| meta.outcomes.iter().filter(|row| row.venue.as_deref() == Some(venue)).count()),
            deployer: deployer.map(|one| one.deployer),
            subs: deployer.map(hl_hip4::Deployer::keys).unwrap_or_default(),
            first: None,
        }
    };
    for market in &catalog.markets {
        let venue = found.entry(market.venue.as_deref()).or_insert_with(|| slot(market.venue.as_deref()));
        venue.markets += 1;
        if market.status == Status::Open {
            venue.open += 1;
        }
        if venue.first.as_ref().is_none_or(|first| market.outcome < first.outcome) {
            venue.first = Some(First { outcome: market.outcome, title: market.title.clone() });
        }
    }
    for question in &catalog.questions {
        found.entry(question.venue.as_deref()).or_insert_with(|| slot(question.venue.as_deref())).questions += 1;
    }
    let mut venues: Vec<Venue> = found.into_values().collect();
    venues.sort_by(|a, b| b.markets.cmp(&a.markets).then_with(|| a.venue.cmp(&b.venue)));
    venues
}

/// One template a venue lists from, and how much.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uses<'a> {
    /// What an outcome or question names after `template:`, version digits and all.
    pub id: String,
    pub family: Family,
    pub outcomes: usize,
    pub questions: usize,
    /// The template as `outcomeTemplates` publishes it; `None` once it no longer does.
    pub template: Option<&'a OutcomeTemplate>,
}

/// The templates a venue's rows and questions were made from, the most used first. A row not
/// made from a template (a fallback, a deployer's own name, the venue's daily markets) counts for
/// none.
#[must_use]
pub fn templates_of<'a>(meta: &OutcomeMeta, venue: Option<&str>, published: &'a [OutcomeTemplate]) -> Vec<Uses<'a>> {
    let mut counted: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in meta.outcomes.iter().filter(|row| row.venue.as_deref() == venue) {
        if let Some(id) = row.name.trim().strip_prefix("template:") {
            counted.entry(id).or_default().0 += 1;
        }
    }
    for question in &meta.questions {
        // A question's venue is its legs', as the catalogue reads it.
        let mut legs = question.named_outcomes.iter().chain(question.fallback_outcome.iter());
        let on = legs.find_map(|leg| meta.outcome(*leg)?.venue.as_deref());
        if on == venue
            && let Some(id) = question.name.trim().strip_prefix("template:")
        {
            counted.entry(id).or_default().1 += 1;
        }
    }
    let mut used: Vec<Uses<'a>> = counted
        .into_iter()
        .map(|(id, (outcomes, questions))| Uses {
            id: id.to_owned(),
            family: Template::classify(&format!("template:{id}")).family,
            outcomes,
            questions,
            template: published.iter().find(|template| template.id == id),
        })
        .collect();
    used.sort_by(|a, b| (b.outcomes + b.questions).cmp(&(a.outcomes + a.questions)).then_with(|| a.id.cmp(&b.id)));
    used
}

#[cfg(test)]
#[path = "../tests/venues.rs"]
mod tests;
