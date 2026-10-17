//! What an outcome deployer signs, all L1 actions: claiming a venue, registering outcomes and questions from
//! templates, settling them with the words `outcomeMeta` lists, and letting other addresses do either. Values
//! are checked when made, not when signed. **Settling pays out at once and cannot be undone.**

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    fmt,
    str::FromStr,
};

use crate::markets::outcomes::meta::{Outcome, OutcomeMeta};
use crate::utils::sign::{L1Action, Map, Value};
use crate::utils::time::Stamp;
use crate::utils::{Address, Dec};

/// Why an action could not be made: what the venue would refuse, or what
/// would pay out the wrong way, before anything is signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeployError {
    #[error("`{0}` is no venue name: a venue is 2 to 4 lower case letters")]
    Venue(String),
    #[error("`spot` is the spot venue's name and cannot be claimed")]
    SpotVenue,
    #[error("a fee scale is a decimal from 0 to 10, which `{0}` is not")]
    FeeScale(String),
    #[error("a settle fraction is a decimal from 0 to 1, which `{0}` is not")]
    Fraction(String),
    #[error("a question needs at least one named outcome")]
    NoNamedOutcomes,
    #[error("setting sub-deployers needs at least one grant")]
    NoGrants,
    #[error("outcome {0} belongs to a question, which settles to 0 or 1 only")]
    NotWhole(u32),
    #[error("outcome {0} is settled twice")]
    SettledTwice(u32),
    #[error("outcome {0} is not in outcomeMeta")]
    NotListed(u32),
    #[error("outcome {0} does not list two sides")]
    NotTwoSided(u32),
    #[error("outcome {outcome} is question {question}'s fallback, which settles with its question")]
    Fallback { outcome: u32, question: u32 },
    #[error("question {0} is not in outcomeMeta")]
    NoQuestion(u32),
    #[error("outcome {outcome} is no named outcome question {question} has left to settle")]
    NotOpen { outcome: u32, question: u32 },
}

/// A venue name: 2 to 4 lower case letters, never `spot`. Activating claims
/// it for good; it stays taken after deactivating. Whether a perp dex holds
/// the name already is for the caller to ask `perpDexs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Venue(String);

impl Venue {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Venue {
    type Err = DeployError;

    /// Refuses rather than lowercases: `AB` is not taken to mean `ab`.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let letters = name.bytes().all(|byte| byte.is_ascii_lowercase());
        if !(2..=4).contains(&name.len()) || !letters {
            return Err(DeployError::Venue(name.to_owned()));
        }
        if name == "spot" {
            return Err(DeployError::SpotVenue);
        }
        Ok(Venue(name.to_owned()))
    }
}

impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `text` as a decimal from 0 to `most`, written with no sign and no
/// exponent. It goes on the wire in normal form: `1.0` as `1`.
fn bounded(text: &str, most: Dec) -> Option<Dec> {
    let text = text.trim();
    if text.starts_with(['+', '-']) {
        return None;
    }
    let value = Dec::parse(text).ok()?;
    (!value.is_negative() && value <= most).then_some(value)
}

/// What a deployer takes of the fees on its outcomes, from 0 to 10.
/// `outcomeMeta` reads a scale back as `1.0`; it is sent as `1`. Only a
/// standalone outcome and a question carry one: a named outcome added later
/// takes its question's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeScale(Dec);

impl FeeScale {
    pub const ZERO: FeeScale = FeeScale(Dec::ZERO);
    pub const ONE: FeeScale = FeeScale(Dec::ONE);

    #[must_use]
    pub const fn value(self) -> Dec {
        self.0
    }

    /// The string hashed: no trailing zeros.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_wire()
    }
}

impl FromStr for FeeScale {
    type Err = DeployError;

    /// Takes `1.0`, `01` or `.50`, and keeps what the venue hashes: `1`, `1`,
    /// `0.5`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        bounded(text, Dec::from_int(10)).map(FeeScale).ok_or_else(|| DeployError::FeeScale(text.trim().to_owned()))
    }
}

impl fmt::Display for FeeScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The share of an outcome's pool its first side is paid when it settles,
/// from 0 to 1: `1` pays the first side in full and `0` the second. A
/// standalone outcome settles to any fraction; a question's only to 0 or 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction(Dec);

impl Fraction {
    pub const ZERO: Fraction = Fraction(Dec::ZERO);
    pub const ONE: Fraction = Fraction(Dec::ONE);

    #[must_use]
    pub const fn value(self) -> Dec {
        self.0
    }

    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_wire()
    }

    /// Exactly 0 or exactly 1, the only fractions a question's outcome takes.
    #[must_use]
    pub fn is_whole(self) -> bool {
        self == Fraction::ZERO || self == Fraction::ONE
    }
}

impl FromStr for Fraction {
    type Err = DeployError;

    /// As [`FeeScale`]'s: `1.0` is kept as `1` and `.50` as `0.5`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        bounded(text, Dec::ONE).map(Fraction).ok_or_else(|| DeployError::Fraction(text.trim().to_owned()))
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The longest value a keyword takes, in bytes; whether the venue counts bytes or characters is not confirmed.
pub const MAX_VALUE_LEN: usize = 100;

/// The longest value a `shortString` keyword takes, counted the same way.
pub const MAX_SHORT_LEN: usize = 10;

/// How far ahead a `dateTime` value may point: 366 days, in milliseconds.
pub const HORIZON_MS: u64 = 366 * 24 * 60 * 60 * 1000;

/// What is wrong with one keyword's value, in words for the person typing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Problem {
    Missing,
    TooLong,
    /// `{`, `}` or `|`, which would break the `keyword:value|...` description
    /// the venue writes from the values.
    Reserved,
    DateTime,
    Date,
    ShortString,
    UInt,
    /// A `uInt` of 2^64 or more.
    TooLarge,
    UDecimal,
    /// A `dateTime` at or before now.
    Passed,
    /// A `dateTime` more than [`HORIZON_MS`] after now.
    TooFar,
    /// A value for a keyword the template does not declare.
    Unknown,
    Repeated,
}

impl Problem {
    /// Said after the keyword's name: `time takes YYYYMMDD-HHMM in UTC`.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Problem::Missing => "needs a value",
            Problem::TooLong => "takes at most 100 characters",
            Problem::Reserved => "cannot hold {, } or |",
            Problem::DateTime => "takes YYYYMMDD-HHMM in UTC",
            Problem::Date => "takes YYYYMMDD in UTC",
            Problem::ShortString => "takes at most 10 characters",
            Problem::UInt => "takes a whole number with no sign or leading zeros",
            Problem::TooLarge => "takes a whole number that fits in 64 bits",
            Problem::UDecimal => "takes a decimal with no sign, no exponent and no leading or trailing zeros",
            Problem::Passed => "has already passed",
            Problem::TooFar => "is more than a year away",
            Problem::Unknown => "is not a keyword of this template",
            Problem::Repeated => "is given twice",
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.words())
    }
}

/// A keyword's value the venue would refuse, and why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{keyword} {problem}")]
pub struct BadValue {
    pub keyword: String,
    pub problem: Problem,
}

impl BadValue {
    fn new(keyword: &str, problem: Problem) -> Self {
        BadValue { keyword: keyword.to_owned(), problem }
    }
}

/// What every value must be, whatever its hint: present, at most
/// [`MAX_VALUE_LEN`] bytes, and free of `{`, `}` and `|`.
fn plain(keyword: &str, value: &str) -> Result<(), BadValue> {
    let problem = if value.is_empty() {
        Problem::Missing
    } else if value.len() > MAX_VALUE_LEN {
        Problem::TooLong
    } else if value.contains(['{', '}', '|']) {
        Problem::Reserved
    } else {
        return Ok(());
    };
    Err(BadValue::new(keyword, problem))
}

/// `0`, or digits that do not start with a zero.
fn is_uint(text: &str) -> bool {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    digits && (text == "0" || !text.starts_with('0'))
}

/// A `uInt`, then optionally a point and digits that do not end in a zero.
fn is_udecimal(text: &str) -> bool {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    is_uint(whole)
        && fraction.is_none_or(|fraction| {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit()) && !fraction.ends_with('0')
        })
}

/// How a template says a keyword's value is written, by the name
/// `outcomeTemplates` gives next to each keyword.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Hint {
    /// `YYYYMMDD-HHMM` in UTC, after now and within a year of it.
    DateTime,
    /// `YYYYMMDD`.
    Date,
    /// At most [`MAX_SHORT_LEN`] bytes.
    ShortString,
    /// A perp's name. Whether one by that name trades is for the caller to
    /// check against the live universe; here it only has to be present.
    HlPerp,
    /// `0`, or digits with no leading zero, below 2^64.
    UInt,
    /// A `uInt`, then optionally a point and digits not ending in a zero.
    UDecimal,
    /// Free text.
    String,
    /// A hint this build does not know, checked as free text and kept under
    /// its own name.
    Other(String),
}

impl Hint {
    /// Never fails: a name this build does not know becomes [`Hint::Other`].
    #[must_use]
    pub fn parse(name: &str) -> Hint {
        match name {
            "dateTime" => Hint::DateTime,
            "date" => Hint::Date,
            "shortString" => Hint::ShortString,
            "hlPerp" => Hint::HlPerp,
            "uInt" => Hint::UInt,
            "uDecimal" => Hint::UDecimal,
            "string" => Hint::String,
            other => Hint::Other(other.to_owned()),
        }
    }

    /// As `outcomeTemplates` spells it.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Hint::DateTime => "dateTime",
            Hint::Date => "date",
            Hint::ShortString => "shortString",
            Hint::HlPerp => "hlPerp",
            Hint::UInt => "uInt",
            Hint::UDecimal => "uDecimal",
            Hint::String => "string",
            Hint::Other(name) => name,
        }
    }

    /// Every rule for `value` that needs no clock. A `dateTime` is only
    /// checked for its shape here; [`Hint::check_at`] adds when it may be.
    pub fn check(&self, keyword: &str, value: &str) -> Result<(), BadValue> {
        plain(keyword, value)?;
        let problem = match self {
            Hint::DateTime if Stamp::exact(value).is_none() => Problem::DateTime,
            Hint::Date if Stamp::day(value).is_none() => Problem::Date,
            Hint::ShortString if value.len() > MAX_SHORT_LEN => Problem::ShortString,
            Hint::UInt if !is_uint(value) => Problem::UInt,
            Hint::UInt if value.parse::<u64>().is_err() => Problem::TooLarge,
            Hint::UDecimal if !is_udecimal(value) => Problem::UDecimal,
            _ => return Ok(()),
        };
        Err(BadValue::new(keyword, problem))
    }

    /// [`Hint::check`], and a `dateTime` after `now_ms` and at most
    /// [`HORIZON_MS`] after it, as the venue asks. A `date` has no rule on
    /// when it may be.
    pub fn check_at(&self, keyword: &str, value: &str, now_ms: u64) -> Result<(), BadValue> {
        self.check(keyword, value)?;
        if *self != Hint::DateTime {
            return Ok(());
        }
        let Some(stamp) = Stamp::exact(value) else {
            return Err(BadValue::new(keyword, Problem::DateTime));
        };
        let at = i128::from(stamp.ms());
        let now = i128::from(now_ms);
        let problem = if at <= now {
            Problem::Passed
        } else if at > now + i128::from(HORIZON_MS) {
            Problem::TooFar
        } else {
            return Ok(());
        };
        Err(BadValue::new(keyword, problem))
    }
}

impl fmt::Display for Hint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A template instance's `keywordToValue`: one value per keyword, kept in the
/// order the venue hashes them, which is bytewise by keyword, so `Zone` comes
/// before `apple`. Built sorted, so no caller can send them otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keywords(BTreeMap<String, String>);

impl Keywords {
    /// No keywords, which a template that declares none is instanced with.
    #[must_use]
    pub const fn new() -> Self {
        Keywords(BTreeMap::new())
    }

    /// Pairs in any order. A keyword given twice is refused, not replaced.
    pub fn from_pairs<K: Into<String>, V: Into<String>>(
        pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self, BadValue> {
        pairs.into_iter().try_fold(Keywords::new(), |keywords, (keyword, value)| keywords.with(keyword, value))
    }

    /// One more pair. A keyword given twice is refused, not replaced.
    pub fn with(mut self, keyword: impl Into<String>, value: impl Into<String>) -> Result<Self, BadValue> {
        let keyword = keyword.into();
        let value = value.into();
        plain(&keyword, &value)?;
        match self.0.entry(keyword) {
            Entry::Occupied(taken) => Err(BadValue::new(taken.key(), Problem::Repeated)),
            Entry::Vacant(free) => {
                free.insert(value);
                Ok(self)
            }
        }
    }

    #[must_use]
    pub fn get(&self, keyword: &str) -> Option<&str> {
        self.0.get(keyword).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The pairs in the order they are hashed in.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(keyword, value)| (keyword.as_str(), value.as_str()))
    }

    /// `[[keyword, value], ...]`, or `[]` for none.
    #[must_use]
    pub fn wire(&self) -> Value {
        Value::array(self.iter().map(|(keyword, value)| Value::array([keyword.to_owned(), value.to_owned()])))
    }

    /// Everything wrong with these as an instance of a template declaring
    /// `declared`, each keyword with its hint: a declared keyword with no
    /// value, a value for a keyword it does not declare, and each value its
    /// hint refuses at `now_ms`. Empty when nothing is.
    #[must_use]
    pub fn check<'a>(&self, declared: impl IntoIterator<Item = (&'a str, Hint)>, now_ms: u64) -> Vec<BadValue> {
        let declared: BTreeMap<&str, Hint> = declared.into_iter().collect();
        let mut problems = Vec::new();
        for (keyword, hint) in &declared {
            match self.get(keyword) {
                None => problems.push(BadValue::new(keyword, Problem::Missing)),
                Some(value) => problems.extend(hint.check_at(keyword, value, now_ms).err()),
            }
        }
        for keyword in self.0.keys() {
            if !declared.contains_key(keyword.as_str()) {
                problems.push(BadValue::new(keyword, Problem::Unknown));
            }
        }
        problems
    }
}

/// A template and a value for each of its keywords: `{"id","keywordToValue"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// The template's id in `outcomeTemplates`, such as `binaryPrice`.
    pub id: String,
    pub keywords: Keywords,
}

impl Instance {
    #[must_use]
    pub fn new(id: impl Into<String>, keywords: Keywords) -> Self {
        Instance { id: id.into(), keywords }
    }

    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("id", self.id.clone()).put("keywordToValue", self.keywords.wire())
    }

    /// With the fee scale a standalone outcome or a question carries, last.
    fn wire_with(&self, fee_scale: FeeScale) -> Map {
        self.wire().put("deployerFeeScale", fee_scale.to_wire())
    }
}

/// One outcome settled: the body of `settleOutcome`, and each entry of
/// `settleQuestion2`. The name, description and side names must be the ones
/// `outcomeMeta` lists, character for character, trailing spaces included
/// ([`Outcome::settlement`] copies them); they are sent as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    pub outcome: u32,
    pub fraction: Fraction,
    pub name: String,
    pub description: String,
    /// The first side is the one `fraction` pays.
    pub sides: [String; 2],
}

impl Settlement {
    /// `details` is always empty: the venue refuses anything else.
    #[must_use]
    pub fn wire(&self) -> Map {
        let [first, second] = &self.sides;
        Map::new()
            .put("outcome", self.outcome)
            .put("settleFraction", self.fraction.to_wire())
            .put("details", "")
            .put("nameAndDescription", Value::array([self.name.clone(), self.description.clone()]))
            .put("sideNames", Value::array([first.clone(), second.clone()]))
    }
}

/// What a sub-deployer may do for a venue: a `setSubDeployers` grant's
/// `variant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    RegisterStandalone,
    RegisterQuestion,
    RegisterAndAssociate,
    SettleOutcome,
    /// Covers `settleQuestion2`, under the older name.
    SettleQuestion,
}

impl Permission {
    pub const ALL: [Permission; 5] = [
        Permission::RegisterStandalone,
        Permission::RegisterQuestion,
        Permission::RegisterAndAssociate,
        Permission::SettleOutcome,
        Permission::SettleQuestion,
    ];

    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Permission::RegisterStandalone => "registerStandaloneOutcomeFromTemplate",
            Permission::RegisterQuestion => "registerQuestionFromTemplate",
            Permission::RegisterAndAssociate => "registerAndAssociateNamedOutcomeFromTemplate",
            Permission::SettleOutcome => "settleOutcome",
            Permission::SettleQuestion => "settleQuestion",
        }
    }
}

/// One address let in to, or shut out of, one [`Permission`] on the venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grant {
    pub permission: Permission,
    pub user: Address,
    pub allowed: bool,
}

impl Grant {
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("variant", self.permission.wire()).put("user", self.user.to_hex()).put("allowed", self.allowed)
    }
}

/// What an `outcomeDeploy` does, under the key naming it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// `registerStandaloneOutcomeFromTemplate`: one outcome with two sides.
    RegisterStandalone { instance: Instance, fee_scale: FeeScale },
    /// `registerQuestionFromTemplate`: a question and its named outcomes, in
    /// the order given, which is the order they are shown in. The venue adds
    /// a fallback, so N named outcomes make N + 1.
    RegisterQuestion { question: Instance, fee_scale: FeeScale, named: Vec<Instance> },
    /// `registerAndAssociateNamedOutcomeFromTemplate`: one more named outcome
    /// on a live question, at the question's fee scale.
    RegisterAndAssociate { question: u32, named: Instance },
    /// `settleOutcome`. Pays out at once and cannot be undone.
    SettleOutcome(Settlement),
    /// `settleQuestion2`: every named outcome of the question not yet
    /// settled, exactly one of them at 1, the fallback left out. Pays out at
    /// once and cannot be undone.
    SettleQuestion { question: u32, settlements: Vec<Settlement>, name: String, description: String },
    /// `setSubDeployers`.
    SetSubDeployers(Vec<Grant>),
}

impl Operation {
    /// The key the operation travels under.
    #[must_use]
    pub const fn wire_name(&self) -> &'static str {
        match self {
            Operation::RegisterStandalone { .. } => "registerStandaloneOutcomeFromTemplate",
            Operation::RegisterQuestion { .. } => "registerQuestionFromTemplate",
            Operation::RegisterAndAssociate { .. } => "registerAndAssociateNamedOutcomeFromTemplate",
            Operation::SettleOutcome(_) => "settleOutcome",
            Operation::SettleQuestion { .. } => "settleQuestion2",
            Operation::SetSubDeployers(_) => "setSubDeployers",
        }
    }

    /// The grant a sub-deployer needs to send this; `None` for
    /// `setSubDeployers`, which no grant covers.
    #[must_use]
    pub const fn permission(&self) -> Option<Permission> {
        match self {
            Operation::RegisterStandalone { .. } => Some(Permission::RegisterStandalone),
            Operation::RegisterQuestion { .. } => Some(Permission::RegisterQuestion),
            Operation::RegisterAndAssociate { .. } => Some(Permission::RegisterAndAssociate),
            Operation::SettleOutcome(_) => Some(Permission::SettleOutcome),
            Operation::SettleQuestion { .. } => Some(Permission::SettleQuestion),
            Operation::SetSubDeployers(_) => None,
        }
    }

    /// Settling pays out at once and cannot be undone.
    #[must_use]
    pub const fn is_settlement(&self) -> bool {
        matches!(self, Operation::SettleOutcome(_) | Operation::SettleQuestion { .. })
    }

    #[must_use]
    pub fn body(&self) -> Value {
        match self {
            Operation::RegisterStandalone { instance, fee_scale } => instance.wire_with(*fee_scale).into(),
            Operation::RegisterQuestion { question, fee_scale, named } => Map::new()
                .put("questionTemplateInstance", question.wire_with(*fee_scale))
                .put("namedOutcomeTemplateInstances", Value::array(named.iter().map(Instance::wire)))
                .into(),
            Operation::RegisterAndAssociate { question, named } => {
                Map::new().put("question", *question).put("namedOutcomeTemplateInstance", named.wire()).into()
            }
            Operation::SettleOutcome(settlement) => settlement.wire().into(),
            Operation::SettleQuestion { question, settlements, name, description } => Map::new()
                .put("question", *question)
                .put("outcomeSettlements", Value::array(settlements.iter().map(Settlement::wire)))
                .put("nameAndDescription", Value::array([name.clone(), description.clone()]))
                .into(),
            Operation::SetSubDeployers(grants) => Value::array(grants.iter().map(Grant::wire)),
        }
    }
}

/// `{"type":"outcomeDeploy","venue","operation":{<name>: ...}}`: every register
/// and settle, sent by the venue's deployer or an address it granted the
/// operation's [`Permission`].
///
/// The constructors check what [`Operation`] alone cannot: that a question
/// has a named outcome, that a grant list is not empty, and that a settlement
/// is one `outcomeMeta` allows, its words copied from it: a question settles
/// every named outcome it has left, exactly one at 1; its fallback never
/// settles alone; and its named outcome settles alone only at 0 or 1.
/// Settling pays out at once and cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeDeploy {
    pub venue: Venue,
    pub operation: Operation,
}

impl OutcomeDeploy {
    #[must_use]
    pub fn register_standalone(venue: Venue, instance: Instance, fee_scale: FeeScale) -> Self {
        OutcomeDeploy { venue, operation: Operation::RegisterStandalone { instance, fee_scale } }
    }

    pub fn register_question(
        venue: Venue,
        question: Instance,
        fee_scale: FeeScale,
        named: Vec<Instance>,
    ) -> Result<Self, DeployError> {
        if named.is_empty() {
            return Err(DeployError::NoNamedOutcomes);
        }
        Ok(OutcomeDeploy { venue, operation: Operation::RegisterQuestion { question, fee_scale, named } })
    }

    #[must_use]
    pub fn register_and_associate(venue: Venue, question: u32, named: Instance) -> Self {
        OutcomeDeploy { venue, operation: Operation::RegisterAndAssociate { question, named } }
    }

    /// `settleOutcome` for `outcome` at `fraction`, its words copied from `meta`. An outcome no
    /// question holds settles to any fraction; a question's named outcome settles alone only to 0
    /// or 1 (testnet settled a question's legs one at a time as each was ruled out); a question's
    /// fallback never settles alone. Pays out at once and cannot be undone.
    pub fn settle_outcome(
        venue: Venue,
        meta: &OutcomeMeta,
        outcome: u32,
        fraction: Fraction,
    ) -> Result<Self, DeployError> {
        let row = meta.outcome(outcome).ok_or(DeployError::NotListed(outcome))?;
        if let Some(question) = meta.question_of(outcome) {
            if question.fallback_outcome == Some(outcome) {
                return Err(DeployError::Fallback { outcome, question: question.question });
            }
            if !fraction.is_whole() {
                return Err(DeployError::NotWhole(outcome));
            }
        }
        let settlement = settlement(row, fraction)?;
        Ok(OutcomeDeploy { venue, operation: Operation::SettleOutcome(settlement) })
    }

    /// `settleQuestion2` for `question`: every named outcome it has left, in the order `meta` lists
    /// them, `winner` at 1 and the rest at 0, the fallback left out, the question's words and each
    /// outcome's copied from `meta`. Pays out at once and cannot be undone.
    pub fn settle_question(venue: Venue, meta: &OutcomeMeta, question: u32, winner: u32) -> Result<Self, DeployError> {
        let asked = meta.question(question).ok_or(DeployError::NoQuestion(question))?;
        if !asked.unsettled().any(|leg| leg == winner) {
            return Err(DeployError::NotOpen { outcome: winner, question });
        }
        let mut seen = BTreeSet::new();
        let mut settlements = Vec::new();
        for leg in asked.unsettled() {
            if !seen.insert(leg) {
                return Err(DeployError::SettledTwice(leg));
            }
            let fraction = if leg == winner { Fraction::ONE } else { Fraction::ZERO };
            let row = meta.outcome(leg).ok_or(DeployError::NotListed(leg))?;
            settlements.push(settlement(row, fraction)?);
        }
        let (name, description) = (asked.name.clone(), asked.description.clone());
        Ok(OutcomeDeploy { venue, operation: Operation::SettleQuestion { question, settlements, name, description } })
    }

    pub fn set_sub_deployers(venue: Venue, grants: Vec<Grant>) -> Result<Self, DeployError> {
        if grants.is_empty() {
            return Err(DeployError::NoGrants);
        }
        Ok(OutcomeDeploy { venue, operation: Operation::SetSubDeployers(grants) })
    }
}

/// A row's settlement at `fraction`, its words as `outcomeMeta` lists them.
fn settlement(row: &Outcome, fraction: Fraction) -> Result<Settlement, DeployError> {
    row.settlement(fraction).ok_or(DeployError::NotTwoSided(row.outcome))
}

impl L1Action for OutcomeDeploy {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "outcomeDeploy")
            .put("venue", self.venue.0.clone())
            .put("operation", Map::new().put(self.operation.wire_name(), self.operation.body()))
    }
}

/// `activateOutcomeDeployer`, which only the account's own key may sign, never an agent. Activating claims the
/// venue name for good and stakes HYPE for at least 183 days; deactivating needs those days behind it and no
/// live outcomes, and cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivateOutcomeDeployer {
    /// `{"activate":{"venueName"}}`.
    Activate(Venue),
    /// `{"deactivate":null}`: the key is sent, with nil.
    Deactivate,
}

impl L1Action for ActivateOutcomeDeployer {
    fn action(&self) -> Map {
        let map = Map::new().put("type", "activateOutcomeDeployer");
        match self {
            ActivateOutcomeDeployer::Activate(venue) => {
                map.put("activate", Map::new().put("venueName", venue.0.clone()))
            }
            ActivateOutcomeDeployer::Deactivate => map.put("deactivate", Value::Nil),
        }
    }
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
