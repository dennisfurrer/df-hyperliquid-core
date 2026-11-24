//! [`CallRequest`]: the call object `eth_call` and `eth_estimateGas` take.

use crate::client::evm::rpc::query::quantity;
use crate::client::evm::tx::Call;
use crate::client::evm::u256::U256;
use crate::utils::{Address, Json, hex};

/// The call object `eth_call` and `eth_estimateGas` take. No `to` estimates
/// or runs a contract creation from `data`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallRequest {
    /// The sender; `None` lets the node pick.
    pub from: Option<Address>,
    /// The contract called; `None` for a creation.
    pub to: Option<Address>,
    /// The gas limit; `None` lets the node pick.
    pub gas: Option<u64>,
    /// Wei of the chain's coin sent with it; `None` sends none.
    pub value: Option<U256>,
    /// The calldata, or the init code of a creation; left out when empty.
    pub data: Vec<u8>,
}

impl CallRequest {
    /// `data` sent to a contract.
    #[must_use]
    pub fn to(to: Address, data: Vec<u8>) -> CallRequest {
        CallRequest { to: Some(to), data, ..CallRequest::default() }
    }

    /// What `call` does, as sent by `from`: what a transaction's gas is estimated with.
    #[must_use]
    pub fn of(from: Option<Address>, call: &Call) -> CallRequest {
        CallRequest {
            from,
            to: call.to,
            gas: None,
            value: (!call.value.is_zero()).then_some(call.value),
            data: call.data.clone(),
        }
    }

    pub(super) fn json(&self) -> Json {
        let mut fields = Vec::new();
        if let Some(from) = self.from {
            fields.push(("from".to_owned(), Json::String(from.to_hex())));
        }
        if let Some(to) = self.to {
            fields.push(("to".to_owned(), Json::String(to.to_hex())));
        }
        if let Some(gas) = self.gas {
            fields.push(("gas".to_owned(), quantity(U256::from_u64(gas))));
        }
        if let Some(value) = self.value {
            fields.push(("value".to_owned(), quantity(value)));
        }
        if !self.data.is_empty() {
            fields.push(("data".to_owned(), Json::String(hex::encode_prefixed(&self.data))));
        }
        Json::Object(fields)
    }
}
