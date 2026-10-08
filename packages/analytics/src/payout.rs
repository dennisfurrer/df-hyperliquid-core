//! Payouts: a file of wallets and USDC amounts paid from a treasury, ported from hip4-tui's
//! payout tool without its files and its sends. A file is read from its text ([`csv`]), recorded
//! as a batch in an append-only ledger of JSON lines ([`ledger`]) that hip4-tui and the payout tool
//! before it share, and planned against the guards a person set ([`config`], [`plan`]) before
//! anything moves. Reading and appending the files, signing and sending are the caller's: every
//! function here takes and gives text and values.
//!
//! A batch id is SHA-256 over the rows and the paying account, so running a file again resumes it
//! rather than paying it twice, and two accounts paying one list are two payouts.

pub mod config;
pub mod csv;
pub mod ledger;
pub mod money;
pub mod plan;

pub use config::{Config, Guard};
pub use csv::{BadSheet, Sheet, batch_id};
pub use ledger::{Batch, Event, Line, Mode, Row, Run, State, Status, Totals, Transfer};
pub use money::BadUsdc;
pub use plan::{Diverted, Expected, Plan, Repeat, Treasury, covers, plan};
