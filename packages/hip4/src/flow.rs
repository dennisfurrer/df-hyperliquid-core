//! A deployer's steps as pure functions, ported from hip4-tui's deploy flow
//! (`tui/src/deploy/outcomes.rs`) without its form: each takes typed input
//! and what it is checked against (a [`Desk`]: `outcomeMeta`, the venue's
//! templates, the account's [`Seat`], the clock and, when read, the venue's
//! limits) and gives the exact action to sign, what it is in words and what
//! to look for in `outcomeMeta` afterwards, or every reason it cannot.
//!
//! Nothing here reads or sends. Settling pays out at once and cannot be
//! undone.

use std::collections::{BTreeMap, BTreeSet};

use hl_core::{Address, L1Action, Map};

use crate::{
    deploy::{
        ActivateOutcomeDeployer, BadValue, DeployError, FeeScale, Fraction, Grant, Instance, Keywords, OutcomeDeploy,
        Permission, Problem, Settlement, Venue,
    },
    meta::{Outcome, OutcomeMeta, Question},
    template::{DeployerLimits, OutcomeTemplate, TemplateRole},
};

/// Who the account is to a venue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seat {
    /// It deploys on no venue, and may activate one.
    Free,
    /// It is the venue's deployer.
    Deployer { venue: String },
    /// Another's venue granted it some operations, by their wire names.
    Granted { venue: String, deployer: Address, may: Vec<String> },
}

impl Seat {
    #[must_use]
    pub fn venue(&self) -> Option<&str> {
        match self {
            Seat::Free => None,
            Seat::Deployer { venue } | Seat::Granted { venue, .. } => Some(venue),
        }
    }

    /// Whether the account may send `operation` on its venue. A grant of
    /// `settleQuestion` covers `settleQuestion2`.
    #[must_use]
    pub fn may(&self, operation: &str) -> bool {
        match self {
            Seat::Free => false,
            Seat::Deployer { .. } => true,
            Seat::Granted { may, .. } => may
                .iter()
                .any(|granted| granted == operation || (granted == "settleQuestion" && operation == "settleQuestion2")),
        }
    }
}

/// The account's seat from `outcomeMeta`'s deployers: its own venue first,
/// then the first venue that granted it anything.
#[must_use]
pub fn seat_of(meta: &OutcomeMeta, account: Address) -> Seat {
    if let Some(venue) = meta.deployers.iter().find(|one| one.deployer == account).and_then(|one| one.venue.clone()) {
        return Seat::Deployer { venue };
    }
    meta.deployers
        .iter()
        .find_map(|one| {
            let may: Vec<String> = one
                .sub_deployers
                .iter()
                .filter(|(_, users)| users.contains(&account))
                .map(|(operation, _)| operation.clone())
                .collect();
            (!may.is_empty()).then(|| Seat::Granted {
                venue: one.venue.clone().unwrap_or_default(),
                deployer: one.deployer,
                may,
            })
        })
        .unwrap_or(Seat::Free)
}

/// Every `outcomeMeta` row on the venue, fallbacks and expired ones included:
/// what its cap of live outcomes counts.
#[must_use]
pub fn live_on(meta: &OutcomeMeta, venue: &str) -> usize {
    meta.outcomes.iter().filter(|row| row.venue.as_deref() == Some(venue)).count()
}

/// What `address` holds on the venue now.
#[must_use]
pub fn held_by(meta: &OutcomeMeta, venue: &str, address: Address) -> Vec<Permission> {
    let Some(deployer) = meta.deployer(venue) else {
        return Vec::new();
    };
    Permission::ALL
        .into_iter()
        .filter(|permission| {
            deployer
                .sub_deployers
                .iter()
                .any(|(operation, users)| operation == permission.wire() && users.contains(&address))
        })
        .collect()
}

/// How the venue writes a row made from a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChain {
    pub name: String,
    pub description: String,
    /// A standalone outcome's sides; a question's outcome has `Yes` and `No`.
    pub sides: Option<[String; 2]>,
}

/// What a row made from `template` with these values reads as in
/// `outcomeMeta`: `template:<id>`, the keywords sorted as `key:value` with
/// bars between, and a standalone outcome's side names under `template:`.
#[must_use]
pub fn on_chain(template: &OutcomeTemplate, values: &BTreeMap<String, String>) -> OnChain {
    let mut pairs: Vec<(&str, &str)> = template
        .keywords
        .iter()
        .map(|keyword| (keyword.name.as_str(), values.get(&keyword.name).map_or("", String::as_str)))
        .collect();
    pairs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let description = pairs.iter().map(|(key, value)| format!("{key}:{value}")).collect::<Vec<_>>().join("|");
    let sides = match &template.role {
        TemplateRole::Standalone { side_names } => match side_names.as_slice() {
            [first, second] => Some([format!("template:{first}"), format!("template:{second}")]),
            _ => None,
        },
        _ => None,
    };
    OnChain { name: format!("template:{}", template.id), description, sides }
}

/// One of the venue's rows still to settle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsettled<'a> {
    /// An outcome no question holds.
    Outcome(&'a Outcome),
    /// A question with a named outcome left.
    Question(&'a Question),
}

impl Unsettled<'_> {
    /// The outcome's id, or the question's.
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Unsettled::Outcome(row) => row.outcome,
            Unsettled::Question(row) => row.question,
        }
    }
}

/// What on `venue` is still to settle: its outcomes outside any question,
/// then its questions with a named outcome left, each by id.
#[must_use]
pub fn unsettled<'a>(meta: &'a OutcomeMeta, venue: &str) -> Vec<Unsettled<'a>> {
    let held: BTreeSet<u32> = meta.questions.iter().flat_map(legs_of).collect();
    let on_venue = |outcome: u32| meta.outcome(outcome).is_some_and(|row| row.venue.as_deref() == Some(venue));
    let mut rows: Vec<Unsettled<'a>> = meta
        .outcomes
        .iter()
        .filter(|row| row.venue.as_deref() == Some(venue) && !held.contains(&row.outcome))
        .map(Unsettled::Outcome)
        .collect();
    rows.sort_by_key(|row| row.id());
    let mut questions: Vec<Unsettled<'a>> =
        questions_on_by(meta, on_venue).into_iter().map(Unsettled::Question).collect();
    questions.sort_by_key(|row| row.id());
    rows.extend(questions);
    rows
}

/// The live questions on `venue`: those with a named outcome left, by id.
#[must_use]
pub fn questions_on<'a>(meta: &'a OutcomeMeta, venue: &str) -> Vec<&'a Question> {
    questions_on_by(meta, |outcome| meta.outcome(outcome).is_some_and(|row| row.venue.as_deref() == Some(venue)))
}

fn questions_on_by(meta: &OutcomeMeta, on_venue: impl Fn(u32) -> bool) -> Vec<&Question> {
    let mut found: Vec<&Question> = meta
        .questions
        .iter()
        .filter(|question| legs_of(question).any(&on_venue) && question.unsettled().next().is_some())
        .collect();
    found.sort_by_key(|question| question.question);
    found
}

fn legs_of(question: &Question) -> impl Iterator<Item = u32> + '_ {
    question.named_outcomes.iter().chain(question.fallback_outcome.iter()).copied()
}

/// A template and what was typed for its keywords.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filled {
    /// The template's id in `outcomeTemplates`.
    pub template: String,
    pub values: BTreeMap<String, String>,
}

impl Filled {
    #[must_use]
    pub fn new<K: Into<String>, V: Into<String>>(
        template: impl Into<String>,
        values: impl IntoIterator<Item = (K, V)>,
    ) -> Filled {
        Filled {
            template: template.into(),
            values: values.into_iter().map(|(key, value)| (key.into(), value.into())).collect(),
        }
    }
}

/// What every step is checked against.
#[derive(Debug, Clone, Copy)]
pub struct Desk<'a> {
    pub meta: &'a OutcomeMeta,
    /// The venue's templates, as `outcomeTemplates` lists them.
    pub templates: &'a [OutcomeTemplate],
    pub seat: &'a Seat,
    /// Milliseconds since 1970, for a `dateTime` keyword.
    pub now_ms: u64,
    /// `outcomeDeployerLimits`, when read: a register is refused past them.
    pub limits: Option<DeployerLimits>,
}

/// The role a template is asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Role {
    Standalone,
    Question,
    /// A named outcome of a question made from `parent`.
    Named {
        parent: String,
    },
}

impl Role {
    fn fits(&self, template: &OutcomeTemplate) -> bool {
        match (self, &template.role) {
            (Role::Standalone, TemplateRole::Standalone { .. }) | (Role::Question, TemplateRole::Question) => true,
            (Role::Named { parent }, TemplateRole::QuestionOutcome { parent: of }) => parent == of,
            _ => false,
        }
    }
}

/// Why a step cannot be signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("this account deploys on no venue; activate one first")]
    NoVenue,
    #[error("the venue's name: {0}")]
    BadVenue(DeployError),
    #[error("{venue} has not granted this account {operation}")]
    NotGranted { venue: String, operation: &'static str },
    #[error("only {venue}'s deployer may do this")]
    NotDeployer { venue: String },
    #[error("this account already deploys on {venue}")]
    HasVenue { venue: String },
    #[error("the venue name {0} is taken")]
    Taken(String),
    #[error("{venue} still lists {live} outcomes; a venue deactivates only with none")]
    LiveOutcomes { venue: String, live: usize },
    #[error("the venue may hold {remaining} more live outcomes and this needs {needed}")]
    Full { needed: u32, remaining: u32 },
    #[error("no template {id} makes {}", .role.words())]
    NoTemplate { id: String, role: Role },
    #[error("{}{bad}", .named.map(|at| format!("named outcome {}: ", at + 1)).unwrap_or_default())]
    Value {
        /// Which named outcome, counted from zero; `None` for the outcome or
        /// question itself.
        named: Option<usize>,
        bad: BadValue,
    },
    #[error(transparent)]
    Deploy(#[from] DeployError),
    #[error("question {0} is no live question of this venue")]
    NoQuestion(u32),
    #[error("outcome {outcome} is not one this venue has left to settle")]
    NotOpen { outcome: u32 },
    #[error("outcome {outcome} is question {question}'s fallback, which settles with its question")]
    Fallback { outcome: u32, question: u32 },
    #[error("outcome {0} does not list two sides")]
    NotTwoSided(u32),
    #[error("nothing changes: every grant asked is already so")]
    NothingChanges,
}

impl Role {
    fn words(&self) -> String {
        match self {
            Role::Standalone => "a standalone outcome".to_owned(),
            Role::Question => "a question".to_owned(),
            Role::Named { parent } => format!("a named outcome of {parent}"),
        }
    }
}

/// What is to be signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Deploy(OutcomeDeploy),
    Activate(ActivateOutcomeDeployer),
}

impl Action {
    /// The action's `type`.
    #[must_use]
    pub const fn request(&self) -> &'static str {
        match self {
            Action::Deploy(_) => "outcomeDeploy",
            Action::Activate(_) => "activateOutcomeDeployer",
        }
    }

    /// Only the account's own key may activate or deactivate: an agent was
    /// refused.
    #[must_use]
    pub const fn own_key_only(&self) -> bool {
        matches!(self, Action::Activate(_))
    }
}

impl L1Action for Action {
    fn action(&self) -> Map {
        match self {
            Action::Deploy(action) => action.action(),
            Action::Activate(action) => action.action(),
        }
    }
}

/// What to look for in `outcomeMeta` once the venue has taken it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Back {
    /// A new row on the venue with this name and description, past `above`.
    Listed { venue: String, name: String, description: String, above: u32 },
    /// The outcome gone from `outcomeMeta`.
    Settled(u32),
    /// The question's named outcomes all settled, or the question gone.
    QuestionSettled(u32),
    /// Each grant as asked.
    Granted { venue: String, grants: Vec<Grant> },
    /// The account the deployer of this venue.
    Activated(String),
    /// The account a deployer of no venue.
    Deactivated,
}

/// What `outcomeMeta` shows of `back`, in words, once it does.
#[must_use]
pub fn found(back: &Back, meta: &OutcomeMeta, account: Address) -> Option<String> {
    match back {
        Back::Listed { venue, name, description, above } => meta
            .outcomes
            .iter()
            .filter(|row| {
                row.outcome > *above
                    && row.venue.as_deref() == Some(venue.as_str())
                    && &row.name == name
                    && &row.description == description
            })
            .map(|row| row.outcome)
            .max()
            .map(|outcome| format!("outcome {outcome} is in outcomeMeta")),
        Back::Settled(outcome) => (meta.outcome(*outcome).is_none())
            .then(|| format!("outcome {outcome} is settled and gone from outcomeMeta")),
        Back::QuestionSettled(question) => meta
            .question(*question)
            .is_none_or(|row| row.unsettled().next().is_none())
            .then(|| format!("question {question} is settled")),
        Back::Granted { venue, grants } => grants
            .iter()
            .all(|grant| held_by(meta, venue, grant.user).contains(&grant.permission) == grant.allowed)
            .then(|| "outcomeMeta shows the grants as asked".to_owned()),
        Back::Activated(venue) => (seat_of(meta, account) == Seat::Deployer { venue: venue.clone() })
            .then(|| format!("the venue {venue} is this account's")),
        Back::Deactivated => {
            (!matches!(seat_of(meta, account), Seat::Deployer { .. })).then(|| "the venue is deactivated".to_owned())
        }
    }
}

/// A step ready to sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// What it does, in words.
    pub what: String,
    pub action: Action,
    pub back: Back,
}

/// The highest outcome id listed, so a new row is told from an old one.
fn highest(meta: &OutcomeMeta) -> u32 {
    meta.outcomes.iter().map(|row| row.outcome).max().unwrap_or(0)
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// The seat's venue, if it may send `operation` there.
fn venue_for(desk: &Desk<'_>, operation: &'static str) -> Result<Venue, Vec<Refusal>> {
    let Some(name) = desk.seat.venue() else {
        return Err(vec![Refusal::NoVenue]);
    };
    let venue = name.parse::<Venue>().map_err(|why| vec![Refusal::BadVenue(why)])?;
    match desk.seat.may(operation) {
        true => Ok(venue),
        false => Err(vec![Refusal::NotGranted { venue: name.to_owned(), operation }]),
    }
}

/// Room for `needed` more live outcomes, when the limits were read.
fn room(desk: &Desk<'_>, needed: u32) -> Result<(), Refusal> {
    match desk.limits {
        Some(limits) if limits.active_remaining < needed => {
            Err(Refusal::Full { needed, remaining: limits.active_remaining })
        }
        _ => Ok(()),
    }
}

fn template<'a>(desk: &Desk<'a>, id: &str, role: Role) -> Result<&'a OutcomeTemplate, Refusal> {
    desk.templates
        .iter()
        .find(|one| one.id == id && role.fits(one))
        .ok_or_else(|| Refusal::NoTemplate { id: id.to_owned(), role })
}

/// The instance `filled` makes of `template`: every keyword it declares with
/// a value its hint takes at `now_ms`, and none it does not declare.
fn instance(
    template: &OutcomeTemplate,
    filled: &Filled,
    now_ms: u64,
    named: Option<usize>,
) -> Result<Instance, Vec<Refusal>> {
    let refuse = |bad: BadValue| Refusal::Value { named, bad };
    let mut problems: Vec<Refusal> = Vec::new();
    let mut keywords = Keywords::new();
    for keyword in &template.keywords {
        let value = filled.values.get(&keyword.name).map_or("", String::as_str);
        let checked = match value.is_empty() {
            true => Err(BadValue { keyword: keyword.name.clone(), problem: Problem::Missing }),
            false => keyword.hint().check_at(&keyword.name, value, now_ms),
        };
        match checked.and_then(|()| keywords.clone().with(keyword.name.clone(), value)) {
            Ok(more) => keywords = more,
            Err(bad) => problems.push(refuse(bad)),
        }
    }
    let declared: BTreeSet<&str> = template.keywords.iter().map(|keyword| keyword.name.as_str()).collect();
    for name in filled.values.keys().filter(|name| !declared.contains(name.as_str())) {
        problems.push(refuse(BadValue { keyword: name.clone(), problem: Problem::Unknown }));
    }
    match problems.is_empty() {
        true => Ok(Instance::new(template.id.clone(), keywords)),
        false => Err(problems),
    }
}

/// `registerStandaloneOutcomeFromTemplate`: one outcome and its two sides.
pub fn register_outcome(desk: &Desk<'_>, outcome: &Filled, fee: FeeScale) -> Result<Plan, Vec<Refusal>> {
    let venue = venue_for(desk, Permission::RegisterStandalone.wire())?;
    let template = template(desk, &outcome.template, Role::Standalone).map_err(|why| vec![why])?;
    room(desk, 1).map_err(|why| vec![why])?;
    let instance = instance(template, outcome, desk.now_ms, None)?;
    let chain = on_chain(template, &outcome.values);
    Ok(Plan {
        what: format!("register a {} outcome on {venue}", template.id),
        back: Back::Listed {
            venue: venue.to_string(),
            name: chain.name,
            description: chain.description,
            above: highest(desk.meta),
        },
        action: Action::Deploy(OutcomeDeploy::register_standalone(venue, instance, fee)),
    })
}

/// `registerQuestionFromTemplate`: a question and its named outcomes, in the
/// order given. The venue adds a fallback, so N named outcomes make N + 1.
pub fn register_question(
    desk: &Desk<'_>,
    question: &Filled,
    fee: FeeScale,
    named: &[Filled],
) -> Result<Plan, Vec<Refusal>> {
    let venue = venue_for(desk, Permission::RegisterQuestion.wire())?;
    let template = template(desk, &question.template, Role::Question).map_err(|why| vec![why])?;
    let needed = u32::try_from(named.len()).unwrap_or(u32::MAX).saturating_add(1);
    room(desk, needed).map_err(|why| vec![why])?;
    let mut problems = Vec::new();
    let asked = instance(template, question, desk.now_ms, None).map_err(|more| problems.extend(more));
    let parent = Role::Named { parent: template.id.clone() };
    let mut instances = Vec::with_capacity(named.len());
    let mut first = None;
    for (at, filled) in named.iter().enumerate() {
        match self::template(desk, &filled.template, parent.clone()) {
            Ok(one) => match instance(one, filled, desk.now_ms, Some(at)) {
                Ok(made) => {
                    first.get_or_insert_with(|| on_chain(one, &filled.values));
                    instances.push(made);
                }
                Err(more) => problems.extend(more),
            },
            Err(why) => problems.push(why),
        }
    }
    let Ok(asked) = asked else {
        return Err(problems);
    };
    if !problems.is_empty() {
        return Err(problems);
    }
    let count = instances.len();
    let action =
        OutcomeDeploy::register_question(venue.clone(), asked, fee, instances).map_err(|why| vec![why.into()])?;
    let first = first.unwrap_or(OnChain { name: String::new(), description: String::new(), sides: None });
    Ok(Plan {
        what: format!("register a {} question with {count} named outcome{} on {venue}", template.id, plural(count)),
        // The question's own row is no outcome, and its fallback is named as
        // every fallback is, so the first named outcome is looked for.
        back: Back::Listed {
            venue: venue.to_string(),
            name: first.name,
            description: first.description,
            above: highest(desk.meta),
        },
        action: Action::Deploy(action),
    })
}

/// `registerAndAssociateNamedOutcomeFromTemplate`: one more named outcome on
/// a live question, from a template its own template takes.
pub fn add_to_question(desk: &Desk<'_>, question: u32, named: &Filled) -> Result<Plan, Vec<Refusal>> {
    let venue = venue_for(desk, Permission::RegisterAndAssociate.wire())?;
    let live = questions_on(desk.meta, venue.as_str())
        .into_iter()
        .find(|live| live.question == question)
        .ok_or_else(|| vec![Refusal::NoQuestion(question)])?;
    let parent = live.name.strip_prefix("template:").unwrap_or(&live.name).to_owned();
    let template = template(desk, &named.template, Role::Named { parent }).map_err(|why| vec![why])?;
    room(desk, 1).map_err(|why| vec![why])?;
    let instance = instance(template, named, desk.now_ms, None)?;
    let chain = on_chain(template, &named.values);
    Ok(Plan {
        what: format!("add a {} outcome to question {question} on {venue}", template.id),
        back: Back::Listed {
            venue: venue.to_string(),
            name: chain.name,
            description: chain.description,
            above: highest(desk.meta),
        },
        action: Action::Deploy(OutcomeDeploy::register_and_associate(venue, question, instance)),
    })
}

/// A row's settlement at `fraction`, its words copied from `outcomeMeta`.
fn settlement(row: &Outcome, fraction: Fraction) -> Result<Settlement, Refusal> {
    row.settlement(fraction).ok_or(Refusal::NotTwoSided(row.outcome))
}

/// `settleOutcome`, its words copied from `outcomeMeta` exactly. An outcome
/// outside any question settles to any fraction; a question's named outcome
/// still open settles alone only to 0 or 1 (testnet settled a question's
/// legs one at a time as each was ruled out); a fallback settles with its
/// question. Pays out at once and cannot be undone.
pub fn settle_outcome(desk: &Desk<'_>, outcome: u32, fraction: Fraction) -> Result<Plan, Vec<Refusal>> {
    let venue = venue_for(desk, Permission::SettleOutcome.wire())?;
    let row = desk
        .meta
        .outcome(outcome)
        .filter(|row| row.venue.as_deref() == Some(venue.as_str()))
        .ok_or_else(|| vec![Refusal::NotOpen { outcome }])?;
    if let Some(question) = desk.meta.question_of(outcome) {
        if question.fallback_outcome == Some(outcome) {
            return Err(vec![Refusal::Fallback { outcome, question: question.question }]);
        }
        if !question.unsettled().any(|leg| leg == outcome) {
            return Err(vec![Refusal::NotOpen { outcome }]);
        }
        if !fraction.is_whole() {
            return Err(vec![Refusal::Deploy(DeployError::NotWhole(outcome))]);
        }
    }
    let settlement = settlement(row, fraction).map_err(|why| vec![why])?;
    Ok(Plan {
        what: format!("settle outcome {outcome} to {fraction} on {venue}"),
        back: Back::Settled(outcome),
        action: Action::Deploy(OutcomeDeploy::settle_outcome(venue, settlement)),
    })
}

/// `settleQuestion2`: every named outcome not yet settled, `winner` at 1 and
/// the rest at 0, the fallback left out, the question's words copied from
/// `outcomeMeta`. Pays out at once and cannot be undone.
pub fn settle_question(desk: &Desk<'_>, question: u32, winner: u32) -> Result<Plan, Vec<Refusal>> {
    let venue = venue_for(desk, "settleQuestion2")?;
    let asked = questions_on(desk.meta, venue.as_str())
        .into_iter()
        .find(|live| live.question == question)
        .ok_or_else(|| vec![Refusal::NoQuestion(question)])?;
    if !asked.unsettled().any(|leg| leg == winner) {
        return Err(vec![Refusal::NotOpen { outcome: winner }]);
    }
    let mut settlements = Vec::new();
    let mut problems = Vec::new();
    for leg in asked.unsettled() {
        let fraction = if leg == winner { Fraction::ONE } else { Fraction::ZERO };
        match desk.meta.outcome(leg).ok_or(Refusal::NotOpen { outcome: leg }).and_then(|row| settlement(row, fraction))
        {
            Ok(made) => settlements.push(made),
            Err(why) => problems.push(why),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let action = OutcomeDeploy::settle_question(
        venue.clone(),
        question,
        settlements,
        asked.name.clone(),
        asked.description.clone(),
    )
    .map_err(|why| vec![why.into()])?;
    Ok(Plan {
        what: format!("settle question {question} with {winner} winning on {venue}"),
        back: Back::QuestionSettled(question),
        action: Action::Deploy(action),
    })
}

/// `setSubDeployers`, only the deployer's: the grants asked that change what
/// `outcomeMeta` shows, and nothing when none would.
pub fn set_grants(desk: &Desk<'_>, grants: &[Grant]) -> Result<Plan, Vec<Refusal>> {
    let Seat::Deployer { venue: name } = desk.seat else {
        return Err(vec![match desk.seat.venue() {
            Some(venue) => Refusal::NotDeployer { venue: venue.to_owned() },
            None => Refusal::NoVenue,
        }]);
    };
    let venue = name.parse::<Venue>().map_err(|why| vec![Refusal::BadVenue(why)])?;
    let changes: Vec<Grant> = grants
        .iter()
        .filter(|grant| held_by(desk.meta, name, grant.user).contains(&grant.permission) != grant.allowed)
        .copied()
        .collect();
    let count = changes.len();
    let action =
        OutcomeDeploy::set_sub_deployers(venue.clone(), changes.clone()).map_err(|_| vec![Refusal::NothingChanges])?;
    Ok(Plan {
        what: format!("change {count} grant{} on {venue}", plural(count)),
        back: Back::Granted { venue: venue.to_string(), grants: changes },
        action: Action::Deploy(action),
    })
}

/// `activateOutcomeDeployer` for an account on no venue, under a name no one
/// holds. The name is claimed for good. The account's own key only.
pub fn activate(desk: &Desk<'_>, venue: Venue) -> Result<Plan, Vec<Refusal>> {
    match desk.seat {
        Seat::Free => {}
        Seat::Deployer { venue: held } => return Err(vec![Refusal::HasVenue { venue: held.clone() }]),
        Seat::Granted { venue: held, .. } => return Err(vec![Refusal::NotDeployer { venue: held.clone() }]),
    }
    if desk.meta.deployer(venue.as_str()).is_some() {
        return Err(vec![Refusal::Taken(venue.to_string())]);
    }
    Ok(Plan {
        what: format!("activate the venue {venue}"),
        back: Back::Activated(venue.to_string()),
        action: Action::Activate(ActivateOutcomeDeployer::Activate(venue)),
    })
}

/// `activateOutcomeDeployer` deactivating the deployer's venue, which needs
/// no live outcomes on it (and 183 days staked, which only the venue knows).
/// It cannot be undone, and the name stays taken. The account's own key only.
pub fn deactivate(desk: &Desk<'_>) -> Result<Plan, Vec<Refusal>> {
    let Seat::Deployer { venue } = desk.seat else {
        return Err(vec![match desk.seat.venue() {
            Some(venue) => Refusal::NotDeployer { venue: venue.to_owned() },
            None => Refusal::NoVenue,
        }]);
    };
    let live = live_on(desk.meta, venue);
    if live > 0 {
        return Err(vec![Refusal::LiveOutcomes { venue: venue.clone(), live }]);
    }
    Ok(Plan {
        what: format!("deactivate the venue {venue}"),
        back: Back::Deactivated,
        action: Action::Activate(ActivateOutcomeDeployer::Deactivate),
    })
}

#[cfg(test)]
#[path = "tests/flow.rs"]
mod tests;
