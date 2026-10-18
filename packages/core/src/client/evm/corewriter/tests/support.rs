//! Test helpers: an [`Action`] built from the docs' fields as a vector lists them, not by the
//! code under test, and bytes as `0x` hex.

use serde_json::Value;

use crate::client::evm::corewriter::{Abstraction, Action, BorrowLendOp, FinalizeVariant, OutcomeOp, Tif};
use crate::client::evm::fixtures;
use crate::utils::Address;

fn n(value: &Value) -> u64 {
    fixtures::text(value).parse().unwrap()
}

fn n128(value: &Value) -> u128 {
    fixtures::text(value).parse().unwrap()
}

fn byte(value: &Value) -> u8 {
    u8::try_from(n(value)).unwrap()
}

fn small(value: &Value) -> u32 {
    u32::try_from(n(value)).unwrap()
}

fn flag(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        other => fixtures::text(other) == "true",
    }
}

fn who(value: &Value) -> Address {
    fixtures::address(fixtures::text(value))
}

/// An action from the docs' fields in the docs' order, as a vector lists them.
pub(crate) fn action(id: u64, v: &[Value]) -> Action {
    match id {
        1 => Action::LimitOrder {
            asset: small(&v[0]),
            is_buy: flag(&v[1]),
            limit_px: n(&v[2]),
            sz: n(&v[3]),
            reduce_only: flag(&v[4]),
            tif: Tif::from_code(byte(&v[5])),
            cloid: n128(&v[6]),
        },
        2 => Action::VaultTransfer { vault: who(&v[0]), is_deposit: flag(&v[1]), usd: n(&v[2]) },
        3 => Action::TokenDelegate { validator: who(&v[0]), wei: n(&v[1]), is_undelegate: flag(&v[2]) },
        4 => Action::StakingDeposit { wei: n(&v[0]) },
        5 => Action::StakingWithdraw { wei: n(&v[0]) },
        6 => Action::SpotSend { destination: who(&v[0]), token: n(&v[1]), wei: n(&v[2]) },
        7 => Action::UsdClassTransfer { ntl: n(&v[0]), to_perp: flag(&v[1]) },
        8 => Action::FinalizeEvmContract {
            token: n(&v[0]),
            variant: FinalizeVariant::from_code(byte(&v[1])),
            create_nonce: n(&v[2]),
        },
        9 => Action::AddApiWallet { wallet: who(&v[0]), name: fixtures::text(&v[1]).to_owned() },
        10 => Action::CancelByOid { asset: small(&v[0]), oid: n(&v[1]) },
        11 => Action::CancelByCloid { asset: small(&v[0]), cloid: n128(&v[1]) },
        12 => Action::ApproveBuilderFee { max_fee_rate: n(&v[0]), builder: who(&v[1]) },
        13 => Action::SendAsset {
            destination: who(&v[0]),
            sub_account: who(&v[1]),
            source_dex: small(&v[2]),
            destination_dex: small(&v[3]),
            token: n(&v[4]),
            wei: n(&v[5]),
        },
        15 => Action::BorrowLend { operation: BorrowLendOp::from_code(byte(&v[0])), token: n(&v[1]), wei: n(&v[2]) },
        16 => Action::SetAbstraction { user: who(&v[0]), abstraction: Abstraction::from_code(byte(&v[1])) },
        17 => Action::Outcome {
            operation: OutcomeOp::from_code(byte(&v[0])),
            question: small(&v[1]),
            outcome: small(&v[2]),
            wei: n(&v[3]),
        },
        other => panic!("no action {other}"),
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    crate::utils::hex::encode_prefixed(bytes)
}
