//! An outcome's status against HyperEVM mainnet's answer and viem's reading of it.

use crate::client::evm::precompiles::support::{s, sent, viem};
use crate::utils::Dec;

use super::*;

#[test]
fn outcomes_read_as_the_chain_wrote_them() {
    let outcome = sent(&outcome_status(1473), "outcomeStatus");
    let expected = viem("outcomeStatus");
    assert_eq!(
        [outcome.status.to_string(), outcome.settled_value.to_string(), outcome.question.to_string()],
        [s(&expected["status"]), s(&expected["settledValue"]), s(&expected["question"])]
    );
    assert_eq!(outcome.state(), OutcomeState::Active);
    assert_eq!(outcome.in_question(), Some(198));
    assert_eq!(outcome.settled(), None, "still trading");
    let settled = OutcomeStatus { status: 2, settled_value: 100_000_000, question: u32::MAX };
    assert_eq!(
        (settled.state(), settled.in_question(), settled.settled()),
        (OutcomeState::Settled, None, Some(Dec::ONE))
    );
    assert_eq!(OutcomeStatus { status: 9, ..settled }.state(), OutcomeState::Other(9));
}
