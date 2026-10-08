//! HyperCore's read precompiles: HyperCore state read from HyperEVM by an
//! `eth_call` to a fixed address, `0x…0800` to `0x…0814`, transcribed from
//! `L1Read.sol` as Hyperliquid's docs attach it
//! (`for-developers/hyperevm/interacting-with-hypercore`) and checked against
//! hip4-tui's table.
//!
//! **The calldata is the ABI-encoded arguments with no selector.**
//! `L1Read.sol` calls each as `ADDRESS.staticcall(abi.encode(...))`, so four
//! bytes of selector would be read as part of the first argument. The answer
//! decodes straight against the declared struct.
//!
//! The numbers are HyperCore's integers, unscaled:
//! - a perp's `markPx`, `oraclePx` and `bbo` are in `10^-(6 - szDecimals)`
//!   ([`perp_price`]), a spot pair's `spotPx` and `bbo` in
//!   `10^-(8 - szDecimals)` of its base token ([`spot_price`]);
//! - a position's `szi` is in the perp's size decimals;
//! - USDC amounts (`entryNtl`, `equity`, `withdrawable`, margins) are 1e6;
//! - token balances are in the token's `weiDecimals`, HYPE staking in 1e8;
//! - an outcome's `settledValue` is 1e8 for 1.0.
//!
//! An invalid input (an unknown asset or vault) is refused, and the call
//! uses all the gas it was given. Gas is `2000 + 65 * (input + output bytes)`.

use hl_core::{Address, Dec, Http};

use crate::{
    abi::{self, AbiError, Token},
    rpc::{BlockTag, CallRequest, Rpc, RpcError},
    u256::{B256, U256},
};

/// One precompile as `L1Read.sol` declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    /// The low two bytes of its address.
    pub at: u16,
    /// The `L1Read.sol` function that wraps it.
    pub name: &'static str,
    pub args: &'static str,
    pub returns: &'static str,
}

const POSITION: &str = "(int64 szi, uint64 entryNtl, int64 isolatedRawUsd, uint32 leverage, bool isIsolated)";

/// Every read precompile `L1Read.sol` names, in address order.
pub const ALL: [Spec; 21] = [
    Spec { at: 0x800, name: "position", args: "address user, uint16 perp", returns: POSITION },
    Spec {
        at: 0x801,
        name: "spotBalance",
        args: "address user, uint64 token",
        returns: "(uint64 total, uint64 hold, uint64 entryNtl)",
    },
    Spec {
        at: 0x802,
        name: "userVaultEquity",
        args: "address user, address vault",
        returns: "(uint64 equity, uint64 lockedUntilTimestamp)",
    },
    Spec { at: 0x803, name: "withdrawable", args: "address user", returns: "(uint64 withdrawable)" },
    Spec {
        at: 0x804,
        name: "delegations",
        args: "address user",
        returns: "(address validator, uint64 amount, uint64 lockedUntilTimestamp)[]",
    },
    Spec {
        at: 0x805,
        name: "delegatorSummary",
        args: "address user",
        returns: "(uint64 delegated, uint64 undelegated, uint64 totalPendingWithdrawal, uint64 nPendingWithdrawals)",
    },
    Spec { at: 0x806, name: "markPx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x807, name: "oraclePx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x808, name: "spotPx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x809, name: "l1BlockNumber", args: "", returns: "uint64" },
    Spec {
        at: 0x80a,
        name: "perpAssetInfo",
        args: "uint32 perp",
        returns: "(string coin, uint32 marginTableId, uint8 szDecimals, uint8 maxLeverage, bool onlyIsolated)",
    },
    Spec { at: 0x80b, name: "spotInfo", args: "uint32 spot", returns: "(string name, uint64[2] tokens)" },
    Spec {
        at: 0x80c,
        name: "tokenInfo",
        args: "uint32 token",
        returns: "(string name, uint64[] spots, uint64 deployerTradingFeeShare, address deployer, address evmContract, uint8 szDecimals, uint8 weiDecimals, int8 evmExtraWeiDecimals)",
    },
    Spec {
        at: 0x80d,
        name: "tokenSupply",
        args: "uint32 token",
        returns: "(uint64 maxSupply, uint64 totalSupply, uint64 circulatingSupply, uint64 futureEmissions, (address user, uint64 balance)[] nonCirculatingUserBalances)",
    },
    Spec { at: 0x80e, name: "bbo", args: "uint32 asset", returns: "(uint64 bid, uint64 ask)" },
    Spec {
        at: 0x80f,
        name: "accountMarginSummary",
        args: "uint32 perp_dex_index, address user",
        returns: "(int64 accountValue, uint64 marginUsed, uint64 ntlPos, int64 rawUsd)",
    },
    Spec { at: 0x810, name: "coreUserExists", args: "address user", returns: "(bool exists)" },
    Spec {
        at: 0x811,
        name: "borrowLendUserState",
        args: "address user, uint64 token",
        returns: "((uint64 basis, uint64 value) borrow, (uint64 basis, uint64 value) supply)",
    },
    Spec {
        at: 0x812,
        name: "borrowLendReserveState",
        args: "uint64 token",
        returns: "(uint64 borrowYearlyRateBps, uint64 supplyYearlyRateBps, uint64 balance, uint64 utilizationBps, uint64 oraclePx, uint64 ltvBps, uint64 totalSupplied, uint64 totalBorrowed)",
    },
    Spec { at: 0x813, name: "position2", args: "address user, uint32 perp", returns: POSITION },
    Spec {
        at: 0x814,
        name: "outcomeStatus",
        args: "uint32 outcome",
        returns: "(uint8 status, uint64 settledValue, uint32 question)",
    },
];

/// The precompile at `0x…<at>`, if `L1Read.sol` names one there.
#[must_use]
pub fn spec(at: u16) -> Option<&'static Spec> {
    ALL.iter().find(|one| one.at == at)
}

/// `0x` and nineteen zero bytes, then the two of `at`.
#[must_use]
pub fn address(at: u16) -> Address {
    let mut bytes = [0u8; 20];
    bytes[18..].copy_from_slice(&at.to_be_bytes());
    Address::from_bytes(bytes)
}

/// A typed read: where the call goes, its calldata, and how its answer reads.
#[derive(Debug, Clone)]
pub struct Read<T> {
    pub at: Address,
    pub data: Vec<u8>,
    decode: fn(&[u8]) -> Result<T, AbiError>,
}

impl<T> Read<T> {
    /// The `eth_call` that makes the read.
    #[must_use]
    pub fn request(&self) -> CallRequest {
        CallRequest::to(self.at, self.data.clone())
    }

    /// The answer's bytes, read against the declared struct.
    pub fn decode(&self, answer: &[u8]) -> Result<T, AbiError> {
        (self.decode)(answer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error("the precompile's answer does not fit its struct: {0}")]
    Abi(#[from] AbiError),
}

impl<H: Http> Rpc<H> {
    /// One read precompile, at the latest block: the only one HyperEVM serves.
    pub async fn read<T>(&self, read: &Read<T>) -> Result<T, ReadError> {
        let answer = self.call(&read.request(), BlockTag::Latest).await?;
        Ok(read.decode(&answer)?)
    }
}

/// One argument word: every precompile argument is a static type, so the
/// calldata is the words one after another, as `abi.encode` writes them.
enum Arg {
    Address(Address),
    Uint(u64),
}

fn read<T>(at: u16, args: &[Arg], decode: fn(&[u8]) -> Result<T, AbiError>) -> Read<T> {
    let mut data = Vec::with_capacity(args.len() * 32);
    for arg in args {
        let word = match arg {
            Arg::Address(address) => B256::from_address(*address).0,
            Arg::Uint(value) => U256::from_u64(*value).to_be_bytes(),
        };
        data.extend_from_slice(&word);
    }
    Read { at: address(at), data, decode }
}

/// The answer of the precompile at `at`, decoded against its declared return.
fn tokens(at: u16, answer: &[u8]) -> Result<Vec<Token>, AbiError> {
    let returns = spec(at).map_or("", |one| one.returns);
    abi::decode(&abi::params(returns)?, answer)
}

/// The fields of a struct answer, read in order.
struct Fields<'a> {
    name: &'static str,
    items: std::slice::Iter<'a, Token>,
}

impl<'a> Fields<'a> {
    /// The fields of the one tuple an answer is.
    fn of(name: &'static str, tokens: &'a [Token]) -> Result<Fields<'a>, AbiError> {
        match tokens {
            [Token::Tuple(items)] => Ok(Fields { name, items: items.iter() }),
            _ => Err(AbiError::Type(format!("{name}: not one struct"))),
        }
    }

    fn take<T>(&mut self, read: impl FnOnce(&'a Token) -> Option<T>) -> Result<T, AbiError> {
        self.items
            .next()
            .and_then(read)
            .ok_or_else(|| AbiError::Type(format!("{}: a field of another type", self.name)))
    }

    fn u64(&mut self) -> Result<u64, AbiError> {
        self.take(Token::as_u64)
    }

    fn u32(&mut self) -> Result<u32, AbiError> {
        self.take(Token::as_u32)
    }

    fn u8(&mut self) -> Result<u8, AbiError> {
        self.take(Token::as_u8)
    }

    fn i64(&mut self) -> Result<i64, AbiError> {
        self.take(Token::as_i64)
    }

    fn i8(&mut self) -> Result<i8, AbiError> {
        self.take(Token::as_i8)
    }

    fn bool(&mut self) -> Result<bool, AbiError> {
        self.take(Token::as_bool)
    }

    fn address(&mut self) -> Result<Address, AbiError> {
        self.take(Token::as_address)
    }

    fn string(&mut self) -> Result<String, AbiError> {
        self.take(|token| token.as_str().map(ToOwned::to_owned))
    }

    fn items(&mut self) -> Result<&'a [Token], AbiError> {
        self.take(Token::as_items)
    }

    fn u64s(&mut self) -> Result<Vec<u64>, AbiError> {
        let name = self.name;
        self.items()?
            .iter()
            .map(|item| item.as_u64().ok_or_else(|| AbiError::Type(format!("{name}: not a list of uint64"))))
            .collect()
    }
}

/// One perp position, as [`position`] and [`position2`] answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Signed size in the perp's size decimals; negative is short.
    pub szi: i64,
    /// Entry notional, USDC at 1e6.
    pub entry_ntl: u64,
    pub isolated_raw_usd: i64,
    pub leverage: u32,
    pub is_isolated: bool,
}

fn decode_position(at: u16, answer: &[u8]) -> Result<Position, AbiError> {
    let tokens = tokens(at, answer)?;
    let mut fields = Fields::of("position", &tokens)?;
    Ok(Position {
        szi: fields.i64()?,
        entry_ntl: fields.u64()?,
        isolated_raw_usd: fields.i64()?,
        leverage: fields.u32()?,
        is_isolated: fields.bool()?,
    })
}

/// `0x800`: one account's position in a perp of the main dex, by a `uint16`
/// index. [`position2`] takes a `uint32`.
#[must_use]
pub fn position(user: Address, perp: u16) -> Read<Position> {
    read(0x800, &[Arg::Address(user), Arg::Uint(u64::from(perp))], |answer| decode_position(0x800, answer))
}

/// `0x813`: [`position`] with a `uint32` perp index, a new address rather than
/// a change to the old one.
#[must_use]
pub fn position2(user: Address, perp: u32) -> Read<Position> {
    read(0x813, &[Arg::Address(user), Arg::Uint(u64::from(perp))], |answer| decode_position(0x813, answer))
}

/// One token's balance on HyperCore's spot side, in its `weiDecimals`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpotBalance {
    pub total: u64,
    /// What open orders hold of it.
    pub hold: u64,
    pub entry_ntl: u64,
}

/// `0x801`: a spot balance. `token` is a token index, or an outcome side's
/// asset id, `100000000 + outcome * 10 + side`.
#[must_use]
pub fn spot_balance(user: Address, token: u64) -> Read<SpotBalance> {
    read(0x801, &[Arg::Address(user), Arg::Uint(token)], |answer| {
        let tokens = tokens(0x801, answer)?;
        let mut fields = Fields::of("spotBalance", &tokens)?;
        Ok(SpotBalance { total: fields.u64()?, hold: fields.u64()?, entry_ntl: fields.u64()? })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserVaultEquity {
    /// USDC at 1e6.
    pub equity: u64,
    /// Unix milliseconds.
    pub locked_until_timestamp: u64,
}

/// `0x802`: an account's equity in a vault, and when it unlocks.
#[must_use]
pub fn user_vault_equity(user: Address, vault: Address) -> Read<UserVaultEquity> {
    read(0x802, &[Arg::Address(user), Arg::Address(vault)], |answer| {
        let tokens = tokens(0x802, answer)?;
        let mut fields = Fields::of("userVaultEquity", &tokens)?;
        Ok(UserVaultEquity { equity: fields.u64()?, locked_until_timestamp: fields.u64()? })
    })
}

/// `0x803`: what the account can withdraw now, USDC at 1e6.
#[must_use]
pub fn withdrawable(user: Address) -> Read<u64> {
    read(0x803, &[Arg::Address(user)], |answer| Fields::of("withdrawable", &tokens(0x803, answer)?)?.u64())
}

/// One stake delegation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delegation {
    pub validator: Address,
    /// HYPE at 1e8.
    pub amount: u64,
    /// Unix milliseconds.
    pub locked_until_timestamp: u64,
}

/// `0x804`: every delegation the account holds.
#[must_use]
pub fn delegations(user: Address) -> Read<Vec<Delegation>> {
    read(0x804, &[Arg::Address(user)], |answer| {
        let tokens = tokens(0x804, answer)?;
        let [Token::Array(items)] = tokens.as_slice() else {
            return Err(AbiError::Type("delegations: not a list".to_owned()));
        };
        items
            .iter()
            .map(|item| {
                let mut fields = Fields::of("delegations", std::slice::from_ref(item))?;
                Ok(Delegation {
                    validator: fields.address()?,
                    amount: fields.u64()?,
                    locked_until_timestamp: fields.u64()?,
                })
            })
            .collect()
    })
}

/// Staking totals, HYPE at 1e8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelegatorSummary {
    pub delegated: u64,
    pub undelegated: u64,
    pub total_pending_withdrawal: u64,
    pub n_pending_withdrawals: u64,
}

/// `0x805`: the account's staking totals.
#[must_use]
pub fn delegator_summary(user: Address) -> Read<DelegatorSummary> {
    read(0x805, &[Arg::Address(user)], |answer| {
        let tokens = tokens(0x805, answer)?;
        let mut fields = Fields::of("delegatorSummary", &tokens)?;
        Ok(DelegatorSummary {
            delegated: fields.u64()?,
            undelegated: fields.u64()?,
            total_pending_withdrawal: fields.u64()?,
            n_pending_withdrawals: fields.u64()?,
        })
    })
}

fn bare(at: u16, answer: &[u8]) -> Result<u64, AbiError> {
    match tokens(at, answer)?.as_slice() {
        [one] => one.as_u64().ok_or_else(|| AbiError::Type("not a uint64".to_owned())),
        _ => Err(AbiError::Type("not one uint64".to_owned())),
    }
}

/// `0x806`: a perp's mark price, in `10^-(6 - szDecimals)` ([`perp_price`]).
#[must_use]
pub fn mark_px(index: u32) -> Read<u64> {
    read(0x806, &[Arg::Uint(u64::from(index))], |answer| bare(0x806, answer))
}

/// `0x807`: a perp's oracle price, scaled as [`mark_px`].
#[must_use]
pub fn oracle_px(index: u32) -> Read<u64> {
    read(0x807, &[Arg::Uint(u64::from(index))], |answer| bare(0x807, answer))
}

/// `0x808`: a spot pair's price, in `10^-(8 - szDecimals)` of its base token
/// ([`spot_price`]). `index` is a pair index, or an outcome side's asset id,
/// `100000000 + outcome * 10 + side`.
#[must_use]
pub fn spot_px(index: u32) -> Read<u64> {
    read(0x808, &[Arg::Uint(u64::from(index))], |answer| bare(0x808, answer))
}

/// `0x809`: HyperCore's block height. The one precompile with no arguments.
#[must_use]
pub fn l1_block_number() -> Read<u64> {
    read(0x809, &[], |answer| bare(0x809, answer))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerpAssetInfo {
    pub coin: String,
    pub margin_table_id: u32,
    pub sz_decimals: u8,
    pub max_leverage: u8,
    pub only_isolated: bool,
}

/// `0x80a`: a perp's name, size decimals and leverage cap.
#[must_use]
pub fn perp_asset_info(perp: u32) -> Read<PerpAssetInfo> {
    read(0x80a, &[Arg::Uint(u64::from(perp))], |answer| {
        let tokens = tokens(0x80a, answer)?;
        let mut fields = Fields::of("perpAssetInfo", &tokens)?;
        Ok(PerpAssetInfo {
            coin: fields.string()?,
            margin_table_id: fields.u32()?,
            sz_decimals: fields.u8()?,
            max_leverage: fields.u8()?,
            only_isolated: fields.bool()?,
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotInfo {
    pub name: String,
    /// The base token's index, then the quote's.
    pub tokens: [u64; 2],
}

/// `0x80b`: a spot pair and the two tokens it is between.
#[must_use]
pub fn spot_info(spot: u32) -> Read<SpotInfo> {
    read(0x80b, &[Arg::Uint(u64::from(spot))], |answer| {
        let tokens = tokens(0x80b, answer)?;
        let mut fields = Fields::of("spotInfo", &tokens)?;
        let name = fields.string()?;
        let pair: [u64; 2] =
            fields.u64s()?.try_into().map_err(|_| AbiError::Type("spotInfo: not two tokens".to_owned()))?;
        Ok(SpotInfo { name, tokens: pair })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    pub name: String,
    /// The spot pairs it trades in.
    pub spots: Vec<u64>,
    pub deployer_trading_fee_share: u64,
    pub deployer: Address,
    /// The linked ERC-20 on HyperEVM; zero when it has none.
    pub evm_contract: Address,
    pub sz_decimals: u8,
    pub wei_decimals: u8,
    /// How many more decimals the ERC-20 counts in than HyperCore does.
    pub evm_extra_wei_decimals: i8,
}

/// `0x80c`: everything about a token, its EVM contract included.
#[must_use]
pub fn token_info(token: u32) -> Read<TokenInfo> {
    read(0x80c, &[Arg::Uint(u64::from(token))], |answer| {
        let tokens = tokens(0x80c, answer)?;
        let mut fields = Fields::of("tokenInfo", &tokens)?;
        Ok(TokenInfo {
            name: fields.string()?,
            spots: fields.u64s()?,
            deployer_trading_fee_share: fields.u64()?,
            deployer: fields.address()?,
            evm_contract: fields.address()?,
            sz_decimals: fields.u8()?,
            wei_decimals: fields.u8()?,
            evm_extra_wei_decimals: fields.i8()?,
        })
    })
}

/// An account holding non-circulating supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserBalance {
    pub user: Address,
    pub balance: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSupply {
    pub max_supply: u64,
    pub total_supply: u64,
    pub circulating_supply: u64,
    pub future_emissions: u64,
    pub non_circulating_user_balances: Vec<UserBalance>,
}

/// `0x80d`: a token's supply, and who holds the part that does not circulate.
#[must_use]
pub fn token_supply(token: u32) -> Read<TokenSupply> {
    read(0x80d, &[Arg::Uint(u64::from(token))], |answer| {
        let tokens = tokens(0x80d, answer)?;
        let mut fields = Fields::of("tokenSupply", &tokens)?;
        let (max_supply, total_supply, circulating_supply, future_emissions) =
            (fields.u64()?, fields.u64()?, fields.u64()?, fields.u64()?);
        let holders = fields
            .items()?
            .iter()
            .map(|item| {
                let mut holder = Fields::of("tokenSupply", std::slice::from_ref(item))?;
                Ok(UserBalance { user: holder.address()?, balance: holder.u64()? })
            })
            .collect::<Result<_, AbiError>>()?;
        Ok(TokenSupply {
            max_supply,
            total_supply,
            circulating_supply,
            future_emissions,
            non_circulating_user_balances: holders,
        })
    })
}

/// The best bid and offer, scaled as the market's prices are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bbo {
    pub bid: u64,
    pub ask: u64,
}

/// `0x80e`: the best bid and offer of a perp (its index) or a spot pair
/// (10000 plus its index).
#[must_use]
pub fn bbo(asset: u32) -> Read<Bbo> {
    read(0x80e, &[Arg::Uint(u64::from(asset))], |answer| {
        let tokens = tokens(0x80e, answer)?;
        let mut fields = Fields::of("bbo", &tokens)?;
        Ok(Bbo { bid: fields.u64()?, ask: fields.u64()? })
    })
}

/// An account's margin on one perp dex, USDC at 1e6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountMarginSummary {
    pub account_value: i64,
    pub margin_used: u64,
    pub ntl_pos: u64,
    pub raw_usd: i64,
}

/// `0x80f`: margin on a perp dex, 0 the main one.
#[must_use]
pub fn account_margin_summary(perp_dex_index: u32, user: Address) -> Read<AccountMarginSummary> {
    read(0x80f, &[Arg::Uint(u64::from(perp_dex_index)), Arg::Address(user)], |answer| {
        let tokens = tokens(0x80f, answer)?;
        let mut fields = Fields::of("accountMarginSummary", &tokens)?;
        Ok(AccountMarginSummary {
            account_value: fields.i64()?,
            margin_used: fields.u64()?,
            ntl_pos: fields.u64()?,
            raw_usd: fields.i64()?,
        })
    })
}

/// `0x810`: whether the address exists on HyperCore at all.
#[must_use]
pub fn core_user_exists(user: Address) -> Read<bool> {
    read(0x810, &[Arg::Address(user)], |answer| Fields::of("coreUserExists", &tokens(0x810, answer)?)?.bool())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasisAndValue {
    pub basis: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowLendUserState {
    pub borrow: BasisAndValue,
    pub supply: BasisAndValue,
}

/// `0x811`: one account's borrow and supply in a token. The precompile
/// refuses an account that has never lent or borrowed it.
#[must_use]
pub fn borrow_lend_user_state(user: Address, token: u64) -> Read<BorrowLendUserState> {
    read(0x811, &[Arg::Address(user), Arg::Uint(token)], |answer| {
        let tokens = tokens(0x811, answer)?;
        let mut fields = Fields::of("borrowLendUserState", &tokens)?;
        let pair = |fields: &mut Fields<'_>| -> Result<BasisAndValue, AbiError> {
            let item = fields.items.next().ok_or_else(|| AbiError::Type("borrowLendUserState: short".to_owned()))?;
            let mut inner = Fields::of("borrowLendUserState", std::slice::from_ref(item))?;
            Ok(BasisAndValue { basis: inner.u64()?, value: inner.u64()? })
        };
        Ok(BorrowLendUserState { borrow: pair(&mut fields)?, supply: pair(&mut fields)? })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowLendReserveState {
    pub borrow_yearly_rate_bps: u64,
    pub supply_yearly_rate_bps: u64,
    pub balance: u64,
    pub utilization_bps: u64,
    pub oracle_px: u64,
    pub ltv_bps: u64,
    pub total_supplied: u64,
    pub total_borrowed: u64,
}

/// `0x812`: a token's lending pool. Its token is a `uint64`, where most
/// neighbours take a `uint32`.
#[must_use]
pub fn borrow_lend_reserve_state(token: u64) -> Read<BorrowLendReserveState> {
    read(0x812, &[Arg::Uint(token)], |answer| {
        let tokens = tokens(0x812, answer)?;
        let mut fields = Fields::of("borrowLendReserveState", &tokens)?;
        Ok(BorrowLendReserveState {
            borrow_yearly_rate_bps: fields.u64()?,
            supply_yearly_rate_bps: fields.u64()?,
            balance: fields.u64()?,
            utilization_bps: fields.u64()?,
            oracle_px: fields.u64()?,
            ltv_bps: fields.u64()?,
            total_supplied: fields.u64()?,
            total_borrowed: fields.u64()?,
        })
    })
}

/// A HIP-4 outcome as the chain sees it, the only HIP-4 read on HyperEVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutcomeStatus {
    pub status: u8,
    /// 1e8 is 1.0; meaningful once settled.
    pub settled_value: u64,
    /// `u32::MAX` when the outcome stands alone or its question is unknown.
    pub question: u32,
}

/// What [`OutcomeStatus::status`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeState {
    NeverExisted,
    Active,
    /// Settled, its value readable.
    Settled,
    /// Settled and pruned: its value is gone.
    Pruned,
    Other(u8),
}

/// The places [`OutcomeStatus::settled_value`] is counted in.
pub const SETTLED_DECIMALS: u32 = 8;

impl OutcomeStatus {
    #[must_use]
    pub const fn state(&self) -> OutcomeState {
        match self.status {
            0 => OutcomeState::NeverExisted,
            1 => OutcomeState::Active,
            2 => OutcomeState::Settled,
            3 => OutcomeState::Pruned,
            other => OutcomeState::Other(other),
        }
    }

    /// The question the outcome belongs to, if any.
    #[must_use]
    pub const fn in_question(&self) -> Option<u32> {
        match self.question {
            u32::MAX => None,
            question => Some(question),
        }
    }

    /// The settlement, 0 to 1, once settled and not pruned.
    #[must_use]
    pub fn settled(&self) -> Option<Dec> {
        match self.state() {
            OutcomeState::Settled => Dec::new(i128::from(self.settled_value), SETTLED_DECIMALS),
            _ => None,
        }
    }
}

/// `0x814`: an outcome's status.
#[must_use]
pub fn outcome_status(outcome: u32) -> Read<OutcomeStatus> {
    read(0x814, &[Arg::Uint(u64::from(outcome))], |answer| {
        let tokens = tokens(0x814, answer)?;
        let mut fields = Fields::of("outcomeStatus", &tokens)?;
        Ok(OutcomeStatus { status: fields.u8()?, settled_value: fields.u64()?, question: fields.u32()? })
    })
}

/// A perp price from [`mark_px`], [`oracle_px`] or [`bbo`]: `raw / 10^(6 - szDecimals)`.
#[must_use]
pub fn perp_price(raw: u64, sz_decimals: u8) -> Option<Dec> {
    Dec::new(i128::from(raw), 6u32.checked_sub(u32::from(sz_decimals))?)
}

/// A spot price from [`spot_px`] or [`bbo`]: `raw / 10^(8 - szDecimals)` of the base token.
#[must_use]
pub fn spot_price(raw: u64, base_sz_decimals: u8) -> Option<Dec> {
    Dec::new(i128::from(raw), 8u32.checked_sub(u32::from(base_sz_decimals))?)
}

#[cfg(test)]
#[path = "tests/precompiles.rs"]
mod tests;
