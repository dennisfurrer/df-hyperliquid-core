//! A live answer cut short of its struct.

use crate::client::evm::precompiles::support::{SHORT, a, live};
use crate::client::evm::precompiles::{bbo, position};

#[test]
fn an_answer_shorter_than_its_struct_is_refused() {
    let (_, _, answer) = live("bbo");
    assert!(bbo(0).decode(&answer[..40]).is_err());
    assert!(position(a(SHORT), 0).decode(&[]).is_err());
}
