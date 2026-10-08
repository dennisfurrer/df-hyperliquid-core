//! CoreWriter, the write path from HyperEVM to HyperCore: a system contract
//! at `0x3333…3333` (`CoreWriter.sol`, attached to the docs'
//! `interacting-with-hypercore` page):
//!
//! ```text
//! event RawAction(address indexed user, bytes data);
//! function sendRawAction(bytes calldata data) external;
//! ```
//!
//! `data` is byte 0 the encoding version, 1; bytes 1 to 3 the action id, a
//! big-endian `uint24`; then the action's fields, ABI-encoded with no
//! selector. The call itself is an ordinary function call, so its calldata is
//! the selector `0x17938e13` and `data` encoded as `bytes`.
//!
//! - The action acts for `msg.sender`: an agent key sending it acts for the
//!   agent's own account, never for the account it trades for.
//! - The transaction's value must be 0: value voids the action silently, with
//!   no log, and HyperCore never sees it.
//! - Orders and vault transfers are delayed on chain by a few seconds.
//!
//! Units: `limitPx` and `sz` are 1e8 whatever the market; `usd` and `ntl`
//! are perp USDC at 1e6; staking and delegation `wei` are HYPE at 1e8; any
//! other `wei` is in the token's `weiDecimals`; `maxFeeRate` is in tenths of a
//! basis point (10 is 0.01%).

use hl_core::{Address, BadUnits, Dec, Network, exact_units};

use crate::{
    abi::{self, AbiError, Token},
    records::Log,
    rpc::{BlockTag, LogFilter},
    tx::{Call, Eip1559, Fees},
    u256::{B256, U256},
};

pub const CORE_WRITER: Address = Address::from_bytes([0x33; 20]);

/// `keccak256("RawAction(address,bytes)")`, the topic every action is logged under.
pub const RAW_ACTION_TOPIC: B256 = B256([
    0x8c, 0x7f, 0x58, 0x5f, 0xb2, 0x95, 0xf7, 0xeb, 0x1e, 0x6a, 0xeb, 0x8f, 0xba, 0x61, 0xb2, 0x3a, 0x4f, 0xe6, 0x0b,
    0xed, 0xa4, 0x05, 0xf0, 0x04, 0x50, 0x73, 0xb1, 0x85, 0xc7, 0x44, 0x12, 0xe3,
]);

/// `sendRawAction(bytes)`.
pub const SEND_RAW_ACTION: [u8; 4] = [0x17, 0x93, 0x8e, 0x13];

/// The only encoding version the docs give.
pub const ENCODING_VERSION: u8 = 1;

/// The one id the docs' table skips.
pub const GAP: u32 = 14;

/// `limitPx` and `sz`: 1e8 times what a person reads, whatever the market.
pub const ORDER_DECIMALS: u32 = 8;
/// `usd` and `ntl`: perp USDC.
pub const USD_DECIMALS: u32 = 6;
/// Staking and delegation `wei`: HYPE.
pub const HYPE_DECIMALS: u32 = 8;
/// `maxFeeRate` in tenths of a basis point, to a percent.
pub const DECIBPS_DECIMALS: u32 = 3;
/// The dex id `sendAsset` gives spot.
pub const SPOT_DEX: u32 = u32::MAX;

/// One row of the docs' action table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub id: u32,
    pub name: &'static str,
    /// The field names, in order, as the docs give them.
    pub fields: &'static [&'static str],
    /// The Solidity types, as the docs give them.
    pub types: &'static str,
}

/// The docs' table, every id it has.
pub const SPECS: [Spec; 16] = [
    Spec {
        id: 1,
        name: "Limit order",
        fields: &["asset", "isBuy", "limitPx", "sz", "reduceOnly", "encodedTif", "cloid"],
        types: "uint32, bool, uint64, uint64, bool, uint8, uint128",
    },
    Spec { id: 2, name: "Vault transfer", fields: &["vault", "isDeposit", "usd"], types: "address, bool, uint64" },
    Spec {
        id: 3,
        name: "Token delegate",
        fields: &["validator", "wei", "isUndelegate"],
        types: "address, uint64, bool",
    },
    Spec { id: 4, name: "Staking deposit", fields: &["wei"], types: "uint64" },
    Spec { id: 5, name: "Staking withdraw", fields: &["wei"], types: "uint64" },
    Spec { id: 6, name: "Spot send", fields: &["destination", "token", "wei"], types: "address, uint64, uint64" },
    Spec { id: 7, name: "USD class transfer", fields: &["ntl", "toPerp"], types: "uint64, bool" },
    Spec {
        id: 8,
        name: "Finalize EVM contract",
        fields: &["token", "encodedFinalizeEvmContractVariant", "createNonce"],
        types: "uint64, uint8, uint64",
    },
    Spec { id: 9, name: "Add API wallet", fields: &["wallet", "name"], types: "address, string" },
    Spec { id: 10, name: "Cancel order by oid", fields: &["asset", "oid"], types: "uint32, uint64" },
    Spec { id: 11, name: "Cancel order by cloid", fields: &["asset", "cloid"], types: "uint32, uint128" },
    Spec { id: 12, name: "Approve builder fee", fields: &["maxFeeRate", "builder"], types: "uint64, address" },
    Spec {
        id: 13,
        name: "Send asset",
        fields: &["destination", "subAccount", "source_dex", "destination_dex", "token", "wei"],
        types: "address, address, uint32, uint32, uint64, uint64",
    },
    Spec {
        id: 15,
        name: "Borrow lend operation",
        fields: &["encodedOperation", "token", "wei"],
        types: "uint8, uint64, uint64",
    },
    Spec { id: 16, name: "Set abstraction", fields: &["user", "abstraction"], types: "address, uint8" },
    Spec {
        id: 17,
        name: "Outcome operation",
        fields: &["encodedOperation", "question", "outcome", "wei"],
        types: "uint8, uint32, uint32, uint64",
    },
];

/// The table's row for `id`.
#[must_use]
pub fn spec(id: u32) -> Option<&'static Spec> {
    SPECS.iter().find(|spec| spec.id == id)
}

/// A limit order's time in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tif {
    Alo,
    Gtc,
    Ioc,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl Tif {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Tif::Alo => 1,
            Tif::Gtc => 2,
            Tif::Ioc => 3,
            Tif::Other(code) => code,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Tif {
        match code {
            1 => Tif::Alo,
            2 => Tif::Gtc,
            3 => Tif::Ioc,
            other => Tif::Other(other),
        }
    }
}

/// How the deployer of an ERC-20 proves it to finalize the link (action 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeVariant {
    /// The deploying address, with the nonce it deployed at.
    Create,
    /// The address in the contract's first storage slot.
    FirstStorageSlot,
    /// The address in the slot at `keccak256("HyperCore deployer")`.
    CustomStorageSlot,
    Other(u8),
}

impl FinalizeVariant {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            FinalizeVariant::Create => 1,
            FinalizeVariant::FirstStorageSlot => 2,
            FinalizeVariant::CustomStorageSlot => 3,
            FinalizeVariant::Other(code) => code,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> FinalizeVariant {
        match code {
            1 => FinalizeVariant::Create,
            2 => FinalizeVariant::FirstStorageSlot,
            3 => FinalizeVariant::CustomStorageSlot,
            other => FinalizeVariant::Other(other),
        }
    }
}

/// A lending operation (action 15). The docs give supply and withdraw; read
/// back from the chain on 2026-08-21 (hip4-tui), 2 routed as borrow and 3 as
/// repay, and 4, 5 and 255 were discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowLendOp {
    Supply,
    Withdraw,
    /// Undocumented.
    Borrow,
    /// Undocumented.
    Repay,
    Other(u8),
}

impl BorrowLendOp {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            BorrowLendOp::Supply => 0,
            BorrowLendOp::Withdraw => 1,
            BorrowLendOp::Borrow => 2,
            BorrowLendOp::Repay => 3,
            BorrowLendOp::Other(code) => code,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> BorrowLendOp {
        match code {
            0 => BorrowLendOp::Supply,
            1 => BorrowLendOp::Withdraw,
            2 => BorrowLendOp::Borrow,
            3 => BorrowLendOp::Repay,
            other => BorrowLendOp::Other(other),
        }
    }
}

/// An account mode (action 16), for the master account or a subaccount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abstraction {
    Disabled,
    UnifiedAccount,
    PortfolioMargin,
    Other(u8),
}

impl Abstraction {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Abstraction::Disabled => 1,
            Abstraction::UnifiedAccount => 2,
            Abstraction::PortfolioMargin => 3,
            Abstraction::Other(code) => code,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Abstraction {
        match code {
            1 => Abstraction::Disabled,
            2 => Abstraction::UnifiedAccount,
            3 => Abstraction::PortfolioMargin,
            other => Abstraction::Other(other),
        }
    }
}

/// A HIP-4 operation (action 17). Unused fields must be zero, or HyperCore
/// drops the payload without a word: split and merge ignore `question`,
/// merge question ignores `outcome`, negate uses both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeOp {
    SplitOutcome,
    MergeOutcome,
    MergeQuestion,
    NegateOutcome,
    Other(u8),
}

impl OutcomeOp {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            OutcomeOp::SplitOutcome => 0,
            OutcomeOp::MergeOutcome => 1,
            OutcomeOp::MergeQuestion => 2,
            OutcomeOp::NegateOutcome => 3,
            OutcomeOp::Other(code) => code,
        }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> OutcomeOp {
        match code {
            0 => OutcomeOp::SplitOutcome,
            1 => OutcomeOp::MergeOutcome,
            2 => OutcomeOp::MergeQuestion,
            3 => OutcomeOp::NegateOutcome,
            other => OutcomeOp::Other(other),
        }
    }
}

/// Every action in the docs' table, its fields in their units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Action 1. `cloid` 0 is none.
    LimitOrder { asset: u32, is_buy: bool, limit_px: u64, sz: u64, reduce_only: bool, tif: Tif, cloid: u128 },
    /// Action 2.
    VaultTransfer { vault: Address, is_deposit: bool, usd: u64 },
    /// Action 3.
    TokenDelegate { validator: Address, wei: u64, is_undelegate: bool },
    /// Action 4.
    StakingDeposit { wei: u64 },
    /// Action 5.
    StakingWithdraw { wei: u64 },
    /// Action 6.
    SpotSend { destination: Address, token: u64, wei: u64 },
    /// Action 7.
    UsdClassTransfer { ntl: u64, to_perp: bool },
    /// Action 8. `create_nonce` is read only by [`FinalizeVariant::Create`].
    FinalizeEvmContract { token: u64, variant: FinalizeVariant, create_nonce: u64 },
    /// Action 9. An empty name makes the wallet the main, unnamed agent.
    AddApiWallet { wallet: Address, name: String },
    /// Action 10.
    CancelByOid { asset: u32, oid: u64 },
    /// Action 11.
    CancelByCloid { asset: u32, cloid: u128 },
    /// Action 12.
    ApproveBuilderFee { max_fee_rate: u64, builder: Address },
    /// Action 13. A non-zero `sub_account` sends from that subaccount; a dex of
    /// [`SPOT_DEX`] is spot.
    SendAsset {
        destination: Address,
        sub_account: Address,
        source_dex: u32,
        destination_dex: u32,
        token: u64,
        wei: u64,
    },
    /// Action 15. `wei` 0 applies the operation to the most there is.
    BorrowLend { operation: BorrowLendOp, token: u64, wei: u64 },
    /// Action 16.
    SetAbstraction { user: Address, abstraction: Abstraction },
    /// Action 17. `wei` must be a multiple of the size; 0 merges the most there is.
    Outcome { operation: OutcomeOp, question: u32, outcome: u32, wei: u64 },
}

/// Why bytes are not a CoreWriter action this reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadAction {
    #[error("not a RawAction log of CoreWriter's")]
    NotRawAction,
    #[error("{0} bytes is shorter than the four-byte header")]
    Short(usize),
    #[error("encoding version {0}; the docs give only 1")]
    Version(u8),
    #[error("action 14, the gap in the docs' table; it has no published meaning")]
    Gap,
    #[error("action {0} is not in the docs' table")]
    UnknownId(u32),
    #[error(transparent)]
    Abi(#[from] AbiError),
}

/// One field's encoding: a word, or text behind an offset.
enum Field {
    Word([u8; 32]),
    Text(Vec<u8>),
}

fn word(value: impl Into<U256>) -> Field {
    Field::Word(value.into().to_be_bytes())
}

fn address(address: Address) -> Field {
    Field::Word(B256::from_address(address).0)
}

fn flag(value: bool) -> Field {
    word(u8::from(value))
}

/// `abi.encode` of the fields: heads, then the tail of the one string.
fn encode(fields: &[Field]) -> Vec<u8> {
    let heads = fields.len() * 32;
    let mut head = Vec::with_capacity(heads);
    let mut tail = Vec::new();
    for field in fields {
        match field {
            Field::Word(word) => head.extend_from_slice(word),
            Field::Text(bytes) => {
                head.extend_from_slice(&U256::from_u64((heads + tail.len()) as u64).to_be_bytes());
                tail.extend_from_slice(&U256::from_u64(bytes.len() as u64).to_be_bytes());
                tail.extend_from_slice(bytes);
                tail.resize(tail.len().div_ceil(32) * 32, 0);
            }
        }
    }
    head.extend(tail);
    head
}

impl Action {
    #[must_use]
    pub const fn id(&self) -> u32 {
        match self {
            Action::LimitOrder { .. } => 1,
            Action::VaultTransfer { .. } => 2,
            Action::TokenDelegate { .. } => 3,
            Action::StakingDeposit { .. } => 4,
            Action::StakingWithdraw { .. } => 5,
            Action::SpotSend { .. } => 6,
            Action::UsdClassTransfer { .. } => 7,
            Action::FinalizeEvmContract { .. } => 8,
            Action::AddApiWallet { .. } => 9,
            Action::CancelByOid { .. } => 10,
            Action::CancelByCloid { .. } => 11,
            Action::ApproveBuilderFee { .. } => 12,
            Action::SendAsset { .. } => 13,
            Action::BorrowLend { .. } => 15,
            Action::SetAbstraction { .. } => 16,
            Action::Outcome { .. } => 17,
        }
    }

    /// The docs' row for this action.
    #[must_use]
    pub fn spec(&self) -> &'static Spec {
        let id = self.id();
        SPECS.iter().find(|spec| spec.id == id).unwrap_or(&SPECS[0])
    }

    fn fields(&self) -> Vec<Field> {
        match self {
            Action::LimitOrder { asset, is_buy, limit_px, sz, reduce_only, tif, cloid } => vec![
                word(*asset),
                flag(*is_buy),
                word(*limit_px),
                word(*sz),
                flag(*reduce_only),
                word(tif.code()),
                word(*cloid),
            ],
            Action::VaultTransfer { vault, is_deposit, usd } => vec![address(*vault), flag(*is_deposit), word(*usd)],
            Action::TokenDelegate { validator, wei, is_undelegate } => {
                vec![address(*validator), word(*wei), flag(*is_undelegate)]
            }
            Action::StakingDeposit { wei } | Action::StakingWithdraw { wei } => vec![word(*wei)],
            Action::SpotSend { destination, token, wei } => vec![address(*destination), word(*token), word(*wei)],
            Action::UsdClassTransfer { ntl, to_perp } => vec![word(*ntl), flag(*to_perp)],
            Action::FinalizeEvmContract { token, variant, create_nonce } => {
                vec![word(*token), word(variant.code()), word(*create_nonce)]
            }
            Action::AddApiWallet { wallet, name } => vec![address(*wallet), Field::Text(name.as_bytes().to_vec())],
            Action::CancelByOid { asset, oid } => vec![word(*asset), word(*oid)],
            Action::CancelByCloid { asset, cloid } => vec![word(*asset), word(*cloid)],
            Action::ApproveBuilderFee { max_fee_rate, builder } => vec![word(*max_fee_rate), address(*builder)],
            Action::SendAsset { destination, sub_account, source_dex, destination_dex, token, wei } => vec![
                address(*destination),
                address(*sub_account),
                word(*source_dex),
                word(*destination_dex),
                word(*token),
                word(*wei),
            ],
            Action::BorrowLend { operation, token, wei } => vec![word(operation.code()), word(*token), word(*wei)],
            Action::SetAbstraction { user, abstraction } => vec![address(*user), word(abstraction.code())],
            Action::Outcome { operation, question, outcome, wei } => {
                vec![word(operation.code()), word(*question), word(*outcome), word(*wei)]
            }
        }
    }

    /// The `data` of `sendRawAction`: the version, the id in three bytes, then
    /// the fields.
    #[must_use]
    pub fn payload(&self) -> Vec<u8> {
        let mut out = vec![ENCODING_VERSION];
        out.extend_from_slice(&self.id().to_be_bytes()[1..]);
        out.extend(encode(&self.fields()));
        out
    }

    /// The whole calldata of the `sendRawAction(bytes)` call.
    #[must_use]
    pub fn calldata(&self) -> Vec<u8> {
        let mut out = SEND_RAW_ACTION.to_vec();
        out.extend(encode(&[Field::Text(self.payload())]));
        out
    }

    /// The call to CoreWriter, with no value: value voids the action.
    #[must_use]
    pub fn call(&self) -> Call {
        Call { to: Some(CORE_WRITER), value: U256::ZERO, data: self.calldata() }
    }

    /// The HyperEVM transaction that sends it on `network`, from the account
    /// it acts for.
    #[must_use]
    pub fn transaction(&self, network: Network, nonce: u64, gas: u64, fees: Fees) -> Eip1559 {
        self.call().transaction(network.evm_chain_id(), nonce, gas, fees)
    }

    /// An action back out of a payload: the header checked, the fields read
    /// against the docs' types.
    pub fn decode(payload: &[u8]) -> Result<Action, BadAction> {
        let Some((header, body)) = payload.split_first_chunk::<4>() else {
            return Err(BadAction::Short(payload.len()));
        };
        if header[0] != ENCODING_VERSION {
            return Err(BadAction::Version(header[0]));
        }
        let id = u32::from_be_bytes([0, header[1], header[2], header[3]]);
        let spec = match (spec(id), id) {
            (Some(spec), _) => spec,
            (None, GAP) => return Err(BadAction::Gap),
            (None, _) => return Err(BadAction::UnknownId(id)),
        };
        let tokens = abi::decode(&abi::params(spec.types)?, body)?;
        let mut fields = Reader { items: tokens.iter() };
        let r = &mut fields;
        Ok(match id {
            1 => Action::LimitOrder {
                asset: r.u32()?,
                is_buy: r.bool()?,
                limit_px: r.u64()?,
                sz: r.u64()?,
                reduce_only: r.bool()?,
                tif: Tif::from_code(r.u8()?),
                cloid: r.u128()?,
            },
            2 => Action::VaultTransfer { vault: r.address()?, is_deposit: r.bool()?, usd: r.u64()? },
            3 => Action::TokenDelegate { validator: r.address()?, wei: r.u64()?, is_undelegate: r.bool()? },
            4 => Action::StakingDeposit { wei: r.u64()? },
            5 => Action::StakingWithdraw { wei: r.u64()? },
            6 => Action::SpotSend { destination: r.address()?, token: r.u64()?, wei: r.u64()? },
            7 => Action::UsdClassTransfer { ntl: r.u64()?, to_perp: r.bool()? },
            8 => Action::FinalizeEvmContract {
                token: r.u64()?,
                variant: FinalizeVariant::from_code(r.u8()?),
                create_nonce: r.u64()?,
            },
            9 => Action::AddApiWallet { wallet: r.address()?, name: r.string()? },
            10 => Action::CancelByOid { asset: r.u32()?, oid: r.u64()? },
            11 => Action::CancelByCloid { asset: r.u32()?, cloid: r.u128()? },
            12 => Action::ApproveBuilderFee { max_fee_rate: r.u64()?, builder: r.address()? },
            13 => Action::SendAsset {
                destination: r.address()?,
                sub_account: r.address()?,
                source_dex: r.u32()?,
                destination_dex: r.u32()?,
                token: r.u64()?,
                wei: r.u64()?,
            },
            15 => Action::BorrowLend { operation: BorrowLendOp::from_code(r.u8()?), token: r.u64()?, wei: r.u64()? },
            16 => Action::SetAbstraction { user: r.address()?, abstraction: Abstraction::from_code(r.u8()?) },
            _ => Action::Outcome {
                operation: OutcomeOp::from_code(r.u8()?),
                question: r.u32()?,
                outcome: r.u32()?,
                wei: r.u64()?,
            },
        })
    }

    /// A limit order, its price and size as a person reads them, scaled to
    /// 1e8. More than eight places is refused, never rounded.
    pub fn limit_order(
        asset: u32,
        is_buy: bool,
        price: Dec,
        size: Dec,
        reduce_only: bool,
        tif: Tif,
        cloid: u128,
    ) -> Result<Action, BadUnits> {
        Ok(Action::LimitOrder {
            asset,
            is_buy,
            limit_px: exact_units(price, ORDER_DECIMALS)?,
            sz: exact_units(size, ORDER_DECIMALS)?,
            reduce_only,
            tif,
            cloid,
        })
    }

    /// USDC into or out of a vault, scaled to 1e6.
    pub fn vault_transfer(vault: Address, is_deposit: bool, usd: Dec) -> Result<Action, BadUnits> {
        Ok(Action::VaultTransfer { vault, is_deposit, usd: exact_units(usd, USD_DECIMALS)? })
    }

    /// USDC between spot and perps, scaled to 1e6.
    pub fn usd_class_transfer(ntl: Dec, to_perp: bool) -> Result<Action, BadUnits> {
        Ok(Action::UsdClassTransfer { ntl: exact_units(ntl, USD_DECIMALS)?, to_perp })
    }

    /// HYPE into staking, scaled to 1e8.
    pub fn staking_deposit(hype: Dec) -> Result<Action, BadUnits> {
        Ok(Action::StakingDeposit { wei: exact_units(hype, HYPE_DECIMALS)? })
    }

    /// HYPE out of staking, scaled to 1e8.
    pub fn staking_withdraw(hype: Dec) -> Result<Action, BadUnits> {
        Ok(Action::StakingWithdraw { wei: exact_units(hype, HYPE_DECIMALS)? })
    }

    /// Staked HYPE to or from a validator, scaled to 1e8.
    pub fn token_delegate(validator: Address, hype: Dec, is_undelegate: bool) -> Result<Action, BadUnits> {
        Ok(Action::TokenDelegate { validator, wei: exact_units(hype, HYPE_DECIMALS)?, is_undelegate })
    }
}

/// The decoded fields of an action, read in order.
struct Reader<'a> {
    items: std::slice::Iter<'a, Token>,
}

impl<'a> Reader<'a> {
    fn take<T>(&mut self, read: impl FnOnce(&'a Token) -> Option<T>) -> Result<T, BadAction> {
        self.items.next().and_then(read).ok_or(BadAction::Abi(AbiError::Type("a field of another type".to_owned())))
    }

    fn u128(&mut self) -> Result<u128, BadAction> {
        self.take(Token::as_u128)
    }

    fn u64(&mut self) -> Result<u64, BadAction> {
        self.take(Token::as_u64)
    }

    fn u32(&mut self) -> Result<u32, BadAction> {
        self.take(Token::as_u32)
    }

    fn u8(&mut self) -> Result<u8, BadAction> {
        self.take(Token::as_u8)
    }

    fn bool(&mut self) -> Result<bool, BadAction> {
        self.take(Token::as_bool)
    }

    fn address(&mut self) -> Result<Address, BadAction> {
        self.take(Token::as_address)
    }

    fn string(&mut self) -> Result<String, BadAction> {
        self.take(|token| token.as_str().map(ToOwned::to_owned))
    }
}

/// A `RawAction` log: who sent it, and what it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAction {
    /// `msg.sender` of `sendRawAction`, the account HyperCore acts for.
    pub user: Address,
    pub version: u8,
    pub id: u32,
    /// The `data` argument as it was logged.
    pub payload: Vec<u8>,
    /// The payload read; an id outside the table keeps the log readable.
    pub action: Result<Action, BadAction>,
}

impl RawAction {
    /// A log of CoreWriter's, read. Any other log is [`BadAction::NotRawAction`].
    pub fn from_log(log: &Log) -> Result<RawAction, BadAction> {
        let user = match log.topics.as_slice() {
            [topic, user, ..] if log.address == CORE_WRITER && *topic == RAW_ACTION_TOPIC => user.to_address(),
            _ => None,
        }
        .ok_or(BadAction::NotRawAction)?;
        let payload = match abi::decode(&abi::params("bytes")?, &log.data)?.as_slice() {
            [Token::Bytes(bytes)] => bytes.clone(),
            _ => return Err(BadAction::NotRawAction),
        };
        let (version, id) = match payload.split_first_chunk::<4>() {
            Some((header, _)) => (header[0], u32::from_be_bytes([0, header[1], header[2], header[3]])),
            None => (0, 0),
        };
        Ok(RawAction { user, version, id, action: Action::decode(&payload), payload })
    }
}

/// The `eth_getLogs` filter for every action sent through CoreWriter between
/// two blocks. HyperEVM serves at most 50 blocks a query.
#[must_use]
pub fn raw_action_logs(from: BlockTag, to: BlockTag) -> LogFilter {
    LogFilter {
        from_block: Some(from),
        to_block: Some(to),
        address: vec![CORE_WRITER],
        topics: vec![Some(vec![RAW_ACTION_TOPIC])],
        ..LogFilter::default()
    }
}

#[cfg(test)]
#[path = "tests/corewriter.rs"]
mod tests;
