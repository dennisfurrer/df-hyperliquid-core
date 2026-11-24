# Migrating to 0.2

From 0.1.0 on, versions follow semantic versioning: a patch release does not remove or rename anything public, and a breaking change waits for the next minor version.

Which part of this guide applies depends on the version you start from:

| From | What changes |
|---|---|
| 0.1.0, 0.1.1 | Enums and the types the crate returns are `#[non_exhaustive]`. See [From 0.1 to 0.2](#from-01-to-02). |
| 0.0.9-beta to 0.0.12-beta | Nothing public was removed or renamed. A few enums gained variants, one method is deprecated and two checks count bytes. See [From 0.0.9-beta or later](#from-009-beta-or-later). |
| 0.0.8-beta, 0.0.8-beta.1 | Modules were reorganised in 0.0.9-beta. Many paths moved and some items were renamed. Read [From 0.0.8-beta](#from-008-beta) first, then the section above. |

From 0.0.x, read the 0.0.x sections first, then [From 0.1 to 0.2](#from-01-to-02).

The lists below were made by comparing the public API of each published version, as rustdoc reports it, and by moving these crates and apps to each release: df-elysium-core, df-papertrade-core, hip4-tui and hip4-vanity, vanity-metal, Atlas and the metalliq SDK.

## Update the dependency

```toml
df-hyperliquid-core = "0.2"
```

The features are unchanged: `sign` and `native` by default, `web` for the browser, `offline` for tests.

From 0.0.11-beta the license is MIT OR Apache-2.0. Earlier versions were BUSL-1.1.

## From 0.1 to 0.2

0.2.0 changes no behaviour and removes nothing. It marks public enums and the structs the crate returns `#[non_exhaustive]`, so that later 0.2.x releases can add a variant or a field without breaking your build.

Outside the crate, `#[non_exhaustive]` means:

- A `match` on one of these enums needs a `_` arm, even when it lists every variant there is today.

  ```rust
  match event {
      Event::Frame(frame) => handle(*frame),
      Event::Closed { reason, .. } => eprintln!("closed: {reason}"),
      _ => {}
  }
  ```

- A pattern that takes such a struct apart needs `..`: `let Level { px, sz, .. } = level;`.
- Such a struct cannot be built with a struct literal, not even with `..Default::default()`. These are answers, frames, decoded reads and plans: you get them from the crate. To build one in a test, read it from the JSON the venue sends, as the crate does:

  ```rust
  let token: SpotToken = serde_json::from_str(
      r#"{"name":"HYPE","index":150,"tokenId":"0x0d01dc56dcaaca66ad901c959b4011ec","weiDecimals":8,"evmContract":null}"#,
  )?;
  ```

- A test that compares a returned struct with a literal compares its fields instead:

  ```rust
  // 0.1
  assert_eq!(remaining, Remaining { daily: 793, active: 3 });
  // 0.2
  assert_eq!((remaining.daily, remaining.active), (793, 3));
  ```

### What stays exhaustive

Enums that cannot gain a variant, so a `match` on them needs no `_` arm: `markets::Side`, `utils::OutcomeSide`, `utils::Round`, `utils::Json`, `markets::Dex`, `client::Network`, `account::Readiness`, `account::OrderLookup`, `client::ExchangeResponse` and `client::Status`. Enum variants with fields are not marked either, so `Event::Closed { reason, retry }` still names every field.

Structs you build yourself keep their struct literals:

- Plan contexts: `transfer::Ctx`, `earn::staking::Ctx`, `earn::vaults::Ctx`, `earn::lending::Ctx`, and what you pass to `Standing::worth`: `account::Beside`, `account::holdings::VaultShare`.
- Orders: `trade::OrderIntent`, `trade::TwapIntent`, `trade::TriggerIntent`, `trade::Exit`, `trade::Close`, `trade::Scale`.
- Transport and HyperEVM: `client::Request`, `client::Response` (for your own `Transport`), `client::evm::Call`, `client::evm::CallRequest`, `client::evm::LogFilter`, `client::evm::Eip1559`, `client::evm::AccessItem`, `client::evm::Function`, `client::evm::Param`.
- Deploys and validators: `deploy::TokenSpec`, `deploy::FinalizeEvmContract`, `deploy::AuthorizeAqav2Role`, `deploy::perp::NewAsset`, `deploy::perp::DexSchema`, `deploy::perp::MarginTier`, `deploy::perp::SubDeployer`, `deploy::perp::DeployerFee`, `deploy::outcomes::Grant`, `deploy::outcomes::Instance`, `deploy::outcomes::OutcomeDeploy`, `deploy::outcomes::Settlement`, `earn::validator::ValidatorProfile`, `earn::validator::ProfileChange`.
- Signing and sockets: `utils::sign::Domain`, `utils::sign::MultiSig`, `live::Timing`, `live::Limits`.
- Small closed values: `markets::outcomes::Template`, `account::AccountRef`, `live::L2Level`, `client::evm::precompiles::Bbo`.

A field added to one of these is a breaking change and waits for the next minor version.

### Deprecated items are still there

`Template::classify`, `SpotDeploy::EnableAlignedQuoteToken` and `SpotDeploy::DisableAlignedQuoteToken` stay deprecated in 0.2.0 and are not removed.

## From 0.0.9-beta or later

### Enums with new variants

These public enums gained variants. A `match` that lists every variant without a `_` arm no longer compiles. Add the new arms, or a `_` arm if you only handle some variants.

| Enum | New variants | Since |
|---|---|---|
| `client::TransportError` | `Offline` | 0.0.10-beta |
| `utils::sign::Field` | `Bytes(&[u8])` | 0.1.0 |
| `account::ActionKind` | `CDeposit2`, `ClaimRewards`, `ConvertToMultiSigUser`, `Hip3LiquidatorTransfer`, `ReserveRequestWeight`, `SendToEvmWithData`, `TrailingStop` | 0.1.0 |
| `trade::TradeAction` | `TrailingStop` | 0.1.0 |
| `live::Subscription` | `L2`, `Pac`, `Sac` | 0.1.0 |
| `live::Frame` | `L2`, `Pac`, `Sac` | 0.1.0 |

`Field::Bytes` is a `bytes` field of any length. A wallet and the posted action both carry it as `0x` and lowercase hex:

```rust
Field::Bytes(bytes) => Json::String(hex::encode_prefixed(bytes)),
```

### `Template::classify` is deprecated

`Template::classify(name)` returns a `Template` and drops a version past 255 without an error. Use `Template::parse(name)`, which returns `Result<Template, VersionTooLarge>`.

In a crate built with `-D warnings`, the deprecation is an error.

```rust
// 0.0.x
let template = Template::classify(&name);

// 0.1
let template = Template::parse(&name)?;
```

If you read names from the venue and must still list a market whose version is past 255, read the name again without its version digits. That gives the family with no version, which is what `classify` returned:

```rust
fn template(name: &str) -> Template {
    Template::parse(name).unwrap_or_else(|_| {
        let without_version = name.trim().trim_end_matches(|c: char| c.is_ascii_digit());
        Template::parse(without_version).unwrap_or(Template { family: Family::Named, version: None })
    })
}
```

### Lengths are counted in UTF-8 bytes

Since 0.1.1, the annotation checks (`setTokenAnnotation`, `setPerpAnnotation`) count UTF-8 bytes, as the venue does. Text with characters outside ASCII that passed before can now be refused before signing. Testnet refused a 6-character display name of 18 bytes.

The HIP-4 keyword checks always counted bytes. Their messages now say so: `priceDescription takes at most 100 bytes`, where 0.0.x said `characters`. Tests that compare the text need the new wording.

### Deprecated in 0.1.1

- `SpotDeploy::EnableAlignedQuoteToken` and `SpotDeploy::DisableAlignedQuoteToken`. Mainnet and testnet refuse both for every shape.

## From 0.0.8-beta

0.0.9-beta reorganised the modules one level deeper. For example, `account` is now split into `account::history`, `account::holdings`, `account::keys` and `account::settings`, and the most used items are re-exported at `account::`.

Most items kept their name and only moved. Import them from their new path, or from the re-export at the top of their module:

```rust
// 0.0.8
use df_hyperliquid_core::account::signer::Signature;

// 0.1
use df_hyperliquid_core::account::Signature;
```

The full list of moved items is in [Moved items](#moved-items). Renamed and removed items, and items whose signature changed, follow it.

### Moved items

These items kept their name and their shape. Only the path changed. Each row lists every 0.0.8 path of the item and the shortest path in 0.1. Items still reachable at one of their 0.0.8 paths, usually through a re-export at the top of their module, are left out because nothing changes for them.

#### `account`

| 0.0.8 | 0.1 |
|---|---|
| `account::Approved`, `account::readiness::Approved` | `trade::Approved` |
| `account::agent::LONGEST_AGENT_NAME` | `account::keys::LONGEST_AGENT_NAME` |
| `account::fees::StakingDiscount` | `account::StakingDiscount` |
| `account::fills::user_fees` | `account::history::user_fees` |
| `account::fills::user_fees_request` | `account::history::user_fees_request` |
| `account::fills::user_fills` | `account::history::user_fills` |
| `account::fills::user_fills_by_time` | `account::history::user_fills_by_time` |
| `account::fills::user_fills_by_time_request` | `account::history::user_fills_by_time_request` |
| `account::fills::user_fills_request` | `account::history::user_fills_request` |
| `account::fills::user_twap_slice_fills` | `account::history::user_twap_slice_fills` |
| `account::fills::user_twap_slice_fills_request` | `account::history::user_twap_slice_fills_request` |
| `account::limits::INITIAL_REQUESTS` | `account::settings::INITIAL_REQUESTS` |
| `account::limits::MOST_OPEN_ORDERS` | `account::settings::MOST_OPEN_ORDERS` |
| `account::limits::OPEN_ORDERS` | `account::settings::OPEN_ORDERS` |
| `account::limits::THROTTLED_EVERY_MS` | `account::settings::THROTTLED_EVERY_MS` |
| `account::limits::VOLUME_PER_OPEN_ORDER` | `account::settings::VOLUME_PER_OPEN_ORDER` |
| `account::limits::address_requests` | `account::settings::address_requests` |
| `account::limits::open_order_cap` | `account::settings::open_order_cap` |
| `account::limits::surplus_cost` | `account::settings::surplus_cost` |
| `account::limits::volume_for` | `account::settings::volume_for` |
| `account::orders::historical_orders` | `account::history::historical_orders` |
| `account::orders::historical_orders_request` | `account::history::historical_orders_request` |
| `account::orders::order_status` | `account::history::order_status` |
| `account::orders::order_status_request` | `account::history::order_status_request` |
| `account::outcomes::Fill` | `account::holdings::outcomes::Fill` |
| `account::outcomes::Holding` | `account::holdings::outcomes::Holding` |
| `account::outcomes::OpenOrder` | `account::holdings::outcomes::OpenOrder` |
| `account::readiness::BUILDER_MODE` | `account::keys::BUILDER_MODE` |
| `account::signer::address_of_public_key` | `account::keys::address_of_public_key` |
| `account::usable::LTV` | `account::holdings::LTV` |
| `account::worth::VaultShare` | `account::holdings::VaultShare` |

#### `client`

| 0.0.8 | 0.1 |
|---|---|
| `client::body::l1` | `client::http::body::l1` |
| `client::body::user_signed` | `client::http::body::user_signed` |
| `client::evm::corewriter::HYPE_DECIMALS` | `utils::HYPE_DECIMALS` |
| `client::limit::BASE_COOLDOWN_MS` | `client::http::limit::BASE_COOLDOWN_MS` |
| `client::limit::Kind` | `client::http::Kind` |
| `client::limit::LIMIT` | `client::http::limit::LIMIT` |
| `client::limit::MAX_COOLDOWN_MS` | `client::http::limit::MAX_COOLDOWN_MS` |
| `client::limit::SOFT` | `client::http::limit::SOFT` |
| `client::limit::WINDOW_MS` | `client::http::limit::WINDOW_MS` |
| `client::limit::action_weight` | `client::http::limit::action_weight` |
| `client::limit::base_weight` | `client::http::limit::base_weight` |
| `client::limit::per_items` | `client::http::limit::per_items` |
| `client::limit::read_weight` | `client::http::limit::read_weight` |
| `client::native::Event`, `client::native::socket::Event` | `live::Event` |
| `client::native::FeedError`, `client::native::socket::FeedError` | `live::FeedError` |
| `client::transport::EXCHANGE` | `client::http::EXCHANGE` |
| `client::transport::INFO` | `client::http::INFO` |

#### `deploy`

| 0.0.8 | 0.1 |
|---|---|
| `deploy::outcomes::flow::OnChain` | `markets::outcomes::OnChain` |

#### `earn`

| 0.0.8 | 0.1 |
|---|---|
| `earn::staking::HYPE_DECIMALS`, `earn::staking::actions::HYPE_DECIMALS` | `utils::HYPE_DECIMALS` |

#### `live`

| 0.0.8 | 0.1 |
|---|---|
| `account::fill::Fill`, `account::fills::Fill`, `live::Fill`, `live::account::Fill` | `account::history::Fill` |
| `account::ledger::LedgerUpdate`, `live::LedgerUpdate`, `live::account::LedgerUpdate`, `transfer::LedgerUpdate`, `transfer::ledger::LedgerUpdate` | `account::history::LedgerUpdate` |
| `account::order::OpenOrder`, `account::orders::OpenOrder`, `live::Order`, `live::account::Order` | `account::history::OpenOrder` |

#### `markets`

| 0.0.8 | 0.1 |
|---|---|
| `markets::reads::candle_snapshot` | `markets::candle::candle_snapshot` |
| `markets::reads::candle_snapshot_request` | `markets::candle::candle_snapshot_request` |
| `markets::reads::l2_book_request` | `markets::book::l2_book_request` |
| `markets::reads::recent_trades_request` | `markets::trades::recent_trades_request` |

#### `trade`

| 0.0.8 | 0.1 |
|---|---|
| `trade::perps::update_leverage` | `trade::margin::update_leverage` |
| `trade::perps::update_leverage_for` | `trade::margin::update_leverage_for` |
| `trade::ticket::LEAST_LEVELS` | `trade::orders::LEAST_LEVELS` |
| `trade::ticket::MINIMUM_TWAP_VALUE` | `trade::orders::MINIMUM_TWAP_VALUE` |
| `trade::ticket::MOST_LEVELS` | `trade::orders::MOST_LEVELS` |
| `trade::ticket::TRIGGER_SLIPPAGE` | `trade::orders::TRIGGER_SLIPPAGE` |
| `trade::ticket::close` | `trade::orders::close` |
| `trade::ticket::leverage` | `trade::margin::leverage` |
| `trade::ticket::margin` | `trade::margin::margin` |
| `trade::ticket::modify` | `trade::manage::modify` |
| `trade::ticket::order` | `trade::orders::order` |
| `trade::ticket::scale` | `trade::orders::scale` |
| `trade::ticket::trigger` | `trade::orders::trigger` |
| `trade::ticket::twap` | `trade::orders::twap` |

#### `transfer`

| 0.0.8 | 0.1 |
|---|---|
| `transfer::arbitrum::BadDeposit` | `transfer::bridge::BadDeposit` |
| `transfer::arbitrum::MAINNET_BRIDGE` | `transfer::bridge::MAINNET_BRIDGE` |
| `transfer::arbitrum::MAINNET_USDC` | `transfer::bridge::MAINNET_USDC` |
| `transfer::arbitrum::MINIMUM_DEPOSIT` | `transfer::bridge::MINIMUM_DEPOSIT` |
| `transfer::arbitrum::TESTNET_BRIDGE` | `transfer::bridge::TESTNET_BRIDGE` |
| `transfer::arbitrum::TESTNET_USDC` | `transfer::bridge::TESTNET_USDC` |
| `transfer::arbitrum::USDC_DECIMALS` | `utils::USDC_DECIMALS` |
| `transfer::arbitrum::bridge` | `transfer::bridge::bridge` |
| `transfer::arbitrum::deposit_calldata` | `transfer::bridge::deposit_calldata` |
| `transfer::arbitrum::rpc_url` | `transfer::bridge::rpc_url` |
| `transfer::arbitrum::usdc` | `transfer::bridge::usdc` |
| `transfer::arbitrum::usdc_transfer_calldata` | `transfer::bridge::usdc_transfer_calldata` |
| `transfer::find`, `transfer::ledger::find` | `account::history::find` |
| `transfer::ledger`, `transfer::ledger::ledger` | `account::history::ledger` |
| `transfer::ledger::ledger_request`, `transfer::ledger_request` | `account::history::ledger_request` |

### Renamed re-exports

In 0.0.8, `live` re-exported three items under other names. Two of the names now belong to other types.

| 0.0.8 | Was | 0.1 |
|---|---|---|
| `live::OpenOrder` | `FrontendOrder` | `account::FrontendOrder`. `account::history::OpenOrder` is a different type. |
| `live::Order` | `account::order::OpenOrder` | `account::history::OpenOrder` |
| `live::Balance` | `SpotBalance` | `account::SpotBalance` |

`account::Approved` moved to `trade::Approved`.

### Renamed and removed items

Each row is one public item of 0.0.8-beta.1 that has no item of the same name and kind in 0.1, or whose same-named item in 0.1 is a different thing. Other paths that re-exported the same item are listed under the first path. "removed" means nothing in 0.1 does the same job.

How to build the argument types that replace a dex name, a size or a place:

- `markets::Dex<'_>`: `Dex::Main` for the main perp dex (what `""` or `None` meant before), `Dex::Hip3("xyz")` for a HIP-3 dex, or `Dex::of_coin("xyz:TSLA")` to take it from a coin.
- `markets::outcomes::Qty`: whole outcome contracts. `Qty::new(5)` from a count, `Qty::from_dec(dec)` from a decimal (refused if it has a fraction), `Qty::parse("5")` from text.
- `place: u32`: a dex's position in `perpDexs`, 0 for the main dex. `markets::dexes::PerpDexs::index_of("xyz")` gives it for a HIP-3 dex, and `markets::dexes::Dexes::dex("xyz")` returns it as the first item of its tuple.

#### `account`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `account::onboarding` (module) | removed | The crate no longer computes onboarding steps. |
| `account::onboarding::Blocked`, `account::Blocked` | removed | |
| `account::onboarding::MIN_DEPOSIT`, `account::MIN_DEPOSIT` | `transfer::bridge::MINIMUM_DEPOSIT` | Same value, 5 USDC: the bridge credits nothing under it. |
| `account::onboarding::Seen`, `account::Seen` | removed | |
| `account::onboarding::Status` | removed | `client::Status` in 0.1 is not this type. |
| `account::onboarding::Step`, `account::Step` | removed | |
| `account::onboarding::StepKind`, `account::StepKind` | removed | |
| `account::onboarding::Wants`, `account::Wants` | removed | |
| `account::onboarding::deposited`, `account::deposited` | removed | It was true when `perp.margin_summary.account_value` is positive or any spot balance row has a positive `total`. |
| `account::onboarding::steps`, `account::steps` | removed | |
| `account::onboarding::left`, `account::left` | removed | |
| `account::agent` (module) | `account::keys::agents` | Same item names; all are also re-exported at `account::`. |
| `account::clearinghouse` (module) | `account::holdings::perps` | `SpotState` and `SpotBalance` are in `account::holdings::spot`, `ActiveAssetData` in `account::holdings::active`; all are re-exported at `account::`. |
| `account::clearinghouse::Clearinghouse` | `account::ClearinghouseState` | Same fields. |
| `account::state::PerpState`, `account::PerpState` (type alias) | `account::ClearinghouseState` | The alias pointed at this type. |
| `account::positions` (module) | `account::holdings::perps` | |
| `account::positions::ClearinghouseState` (type alias) | `account::ClearinghouseState` | Now a struct, not an alias; same type and fields, so callers need no change. |
| `account::state::SpotRisk`, `account::SpotRisk` (trait) | method `SpotState::risk` | `spot.risk()` works as before; remove the trait import. |
| `account::perp_state` | `account::clearinghouse_state` | `perp_state(t, user)` becomes `clearinghouse_state(t, user, Dex::Main)`. |
| `account::perp_state_on` | `account::clearinghouse_state` | `perp_state_on(t, user, dex)` becomes `clearinghouse_state(t, user, d)` with `d` as `Dex::Main` for `""` and `Dex::Hip3(dex)` otherwise. |
| `account::state::perp_state_request`, `account::perp_state_request` | `account::clearinghouse_state_request` | Pass `Dex::Main` as the second argument. |
| `account::state::perp_state_on_request`, `account::perp_state_on_request` | `account::clearinghouse_state_request` | The `&str` dex becomes a `Dex<'_>`. |
| `account::dollars` (function and module) | removed | It read the mode, spot and the main dex and added spot USDC `total` to perp `withdrawable`, counting perp as zero when `Abstraction::is_pooled` is true. Do the same with `account::abstraction`, `account::spot_state` and `account::clearinghouse_state`. |
| `account::dollars::Dollars`, `account::Dollars` | removed | See `account::dollars`. |
| `account::fees`, `account::fees::fees` (function; also the module `account::fees`) | `account::history::user_fees` | Returns `account::UserFees` instead of `Fees`. |
| `account::fees::fees_request`, `account::fees_request` | `account::history::user_fees_request` | |
| `account::fees::Fees` | `account::UserFees` | `daily_user_vlm` holds `DailyVolume`; `fee_schedule` is a `FeeSchedule`, no longer an `Option`. `volume()` is kept. |
| `account::fees::FeeSchedule` | `account::FeeSchedule` | `referral_discount` is a `Dec` (was `Option<Dec>`); adds `tiers` and `staking_discount_tiers`. |
| `account::fees::DayVolume` | `account::DailyVolume` | Same fields. |
| `account::fees::bps` | removed | It multiplied a rate by 10000: `rate.checked_mul(Dec::from_int(10_000))`. |
| `account::fill` (module) | `account::history::fills` | `Fill`, `FillLiquidation`, `TwapSliceFill`. |
| `account::ledger` (module) | `account::history::ledger` | |
| `account::ledger::Delta` | `account::history::ledger::Delta` | Adds the variants `BorrowLend` and `Liquidation`; a match over every variant needs arms for them. |
| `account::order` (module) | `account::history::orders` | `OpenOrder`, `FrontendOrder`, `OrderKind`, `OrderRecord`, `OrderState`. `Cloid`, `Tif` and `Tpsl` are `trade::Cloid`, `trade::Tif`, `trade::Tpsl`. |
| `account::orders::open_orders` | `account::history::open_orders` | `on: Option<&str>` becomes `dex: Dex<'_>`: `None` is `Dex::Main`, `Some(name)` is `Dex::Hip3(name)`. |
| `account::orders::open_orders_request` | `account::history::open_orders_request` | Same change of the dex argument. |
| `account::orders::frontend_open_orders` | `account::history::frontend_open_orders` | Same change of the dex argument. |
| `account::orders::frontend_open_orders_request` | `account::history::frontend_open_orders_request` | Same change of the dex argument. |
| `account::sub_account` (module) | `account::settings::subaccounts` | |
| `account::limits::Budget`, `account::Budget` | removed | |
| `account::limits::Sample`, `account::Sample` | removed | |
| `account::limits::READ_BATCH` | removed | Value was 10. |
| `account::limits::READ_EVERY_MS` | removed | Value was 60000. |
| `account::limits::read_wait_ms` | removed | It returned `read_at + 60000 - now` while that is in the future. |
| `account::outcomes::Check`, `account::Check` | removed | |
| `account::outcomes::corroborate`, `account::corroborate` | removed | |
| `account::outcomes::Position` | removed | Positions derived from fills are no longer computed. `account::Position` in 0.1 is a perp position. |
| `account::outcomes::positions` | removed | |
| `account::outcomes::Summary`, `account::Summary` | removed | |
| `account::outcomes::summarize`, `account::summarize` | removed | |
| `account::outcomes::RawFill`, `account::RawFill` (type alias) | `account::history::Fill` | The alias pointed at this type. |
| `account::outcomes::RawOrder`, `account::RawOrder` (type alias) | `account::history::OpenOrder` | The alias pointed at this type. |
| `account::outcomes::SpotBalances`, `account::SpotBalances` (type alias) | `account::SpotState` | |
| `account::outcomes::SpotRow`, `account::SpotRow` (type alias) | `account::SpotBalance` | |
| `account::outcomes::holdings` | `account::holdings::outcomes::holdings` | Same signature: takes `&SpotState`, returns `Vec<Holding>`. |
| `account::readiness::Clearance`, `account::Clearance` | folded into `trade::BuilderSetting::approval` | `setting.approval(&order, &approved, key)` returns `Result<trade::Approval, trade::ApprovalRefused>`. `Clear` is `Approval::Goes`, `Approve { approved }` is `Approval::ApproveFirst { approved, rate }`, `Refused` is `Err(ApprovalRefused::NotRead)` or `Err(ApprovalRefused::Unreadable(..))`. |
| `account::readiness::clearance`, `account::clearance` | folded into `trade::BuilderSetting::approval` | Takes the order, the `trade::Approved` and the signing `account::Key` instead of a fee. |
| `account::usable::Draws`, `account::Draws` | removed | Removed with `Keeping::draws`. The transfer plans refuse a move between perps and spot on a pooled account with `transfer::Refused::OneBalance`; `Keeping::pools()` says whether an account is pooled. |
| `account::usable::Move`, `account::Move` | removed | See `account::usable::Draws`. |
| `account::usable::MOST_USERS` | `live::MAX_USERS` | Same value, 10. |
| `account::usable::Standings`, `account::Standings` | removed | |
| `account::usable::USDC` | `client::USDC_INDEX` | Same value, 0. |
| `account::worth::Holdings`, `account::Holdings` | removed | |
| `account::worth::Holding` | removed | `account::holdings::outcomes::Holding` in 0.1 is an outcome side, not this type. |
| `account::worth::holdings` | removed | |
| `account::worth::Health` | removed | Read lending health from `earn::lending::UserState`: `risk()` and `factor()`. |
| `account::worth::LendingHealth` | removed | See `account::worth::Health`. |
| `account::worth::Reserve` | removed | `earn::lending::UserState::supplied(token)` and `borrowed(token)` give the same figures. |

#### `client`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `client::api` (module) | removed | Now private. |
| `client::api::Api`, `client::Api` | removed | Now private. Use `client::Client::reader(http, network, clock)`: it is a `Transport` with the same rate-limit gate, and has `network()`, `http()` and `spent()`. |
| `client::limit::Gate` | removed | Now private; `Client` applies it. `Client::spent()` returns `client::Spent`. |
| `client::limit::Meter` | removed | As `Gate`. |
| `client::limit::Ticket` | removed | As `Gate`. |
| `client::evm::chain` (module) | removed | Chain ids come from `client::Network`. |
| `client::evm::chain::Chain`, `client::evm::Chain` | removed | `Chain::HyperEvm(n).id()` is `n.evm_chain_id()`; `Chain::Arbitrum(n).id()` is `n.arbitrum_chain_id()`. Names and gas coins are not provided. `NATIVE_DECIMALS` is now `client::evm::NATIVE_DECIMALS`. |
| `client::evm::chain::ARBITRUM_ONE`, `client::evm::ARBITRUM_ONE` | method `Network::arbitrum_chain_id` | `Network::Mainnet.arbitrum_chain_id()`, 42161. |
| `client::evm::chain::ARBITRUM_SEPOLIA`, `client::evm::ARBITRUM_SEPOLIA` | method `Network::arbitrum_chain_id` | `Network::Testnet.arbitrum_chain_id()`, 421614. |
| `client::evm::corewriter::DECIBPS_DECIMALS` | removed | Value was 3: `maxFeeRate` is in tenths of a basis point. |
| `client::evm::corewriter::USD_DECIMALS` | `utils::USDC_DECIMALS` | Same value, 6. |
| `client::evm::words` (module) | removed | Removed with `Action::describe`. |
| `client::evm::words::Names`, `client::evm::Names` | removed | |
| `client::evm::words::TokenName` | removed | |
| `client::evm::words::Market` | removed | |
| `client::evm::words::short` | removed | It wrote an address as its first 6 and last 4 hex characters joined by `..`. |

#### `deploy`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `deploy::dexes::DeployAuction`, `deploy::DeployAuction` | `deploy::GasAuction` | It was another name for the same type. |
| `deploy::outcomes::flow` (module) | removed | The checked steps are gone. Build actions with `deploy::outcomes::OutcomeDeploy` and `deploy::outcomes::ActivateOutcomeDeployer`; the checks against templates, limits and grants are no longer made. |
| `deploy::outcomes::flow::register_outcome` | `deploy::outcomes::OutcomeDeploy::register_standalone` | `register_standalone(venue, instance, fee_scale)` returns the action itself. |
| `deploy::outcomes::flow::register_question` | `deploy::outcomes::OutcomeDeploy::register_question` | `register_question(venue, question, fee_scale, named)`; refuses an empty `named`. |
| `deploy::outcomes::flow::add_to_question` | `deploy::outcomes::OutcomeDeploy::register_and_associate` | `register_and_associate(venue, question, named)`. |
| `deploy::outcomes::flow::settle_outcome` | `deploy::outcomes::OutcomeDeploy::settle_outcome` | `settle_outcome(venue, &meta, outcome, fraction)`: still checked against `outcomeMeta`. |
| `deploy::outcomes::flow::settle_question` | `deploy::outcomes::OutcomeDeploy::settle_question` | `settle_question(venue, &meta, question, winner)`. |
| `deploy::outcomes::flow::set_grants` | `deploy::outcomes::OutcomeDeploy::set_sub_deployers` | `set_sub_deployers(venue, grants)`; it no longer drops grants that change nothing. |
| `deploy::outcomes::flow::activate` | `deploy::outcomes::ActivateOutcomeDeployer::Activate` | `ActivateOutcomeDeployer::Activate(venue)`. |
| `deploy::outcomes::flow::deactivate` | `deploy::outcomes::ActivateOutcomeDeployer::Deactivate` | |
| `deploy::outcomes::flow::Filled` | `deploy::outcomes::Instance` | `Filled::new(id, pairs)` becomes `Instance::new(id, Keywords::from_pairs(pairs)?)`. |
| `deploy::outcomes::flow::on_chain` | method `markets::outcomes::OutcomeTemplate::on_chain` | `template.on_chain(&keywords)` returns `markets::outcomes::OnChain`. |
| `deploy::outcomes::flow::unsettled` | removed | `markets::outcomes::Question::unsettled()` lists the named outcomes a question has left. |
| `deploy::outcomes::flow::Unsettled` | removed | |
| `deploy::outcomes::flow::questions_on` | removed | |
| `deploy::outcomes::flow::live_on` | removed | It counted the `outcomeMeta` outcomes on the venue. |
| `deploy::outcomes::flow::held_by` | removed | `markets::outcomes::Deployer::sub_deployers` lists each operation and the addresses granted it. |
| `deploy::outcomes::flow::seat_of` | removed | |
| `deploy::outcomes::flow::Seat` | removed | |
| `deploy::outcomes::flow::Desk` | removed | |
| `deploy::outcomes::flow::Back` | removed | |
| `deploy::outcomes::flow::found` | removed | |
| `deploy::outcomes::flow::Role` | removed | |
| `deploy::outcomes::flow::Refusal` | removed | Refusals left are `deploy::outcomes::DeployError`. |
| `deploy::outcomes::flow::Action` | removed | |
| `deploy::outcomes::flow::Plan` | removed | `transfer::Plan` in 0.1 is not this type. |

#### `earn`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `earn::vaults::transfer::USD_DECIMALS`, `earn::vaults::USD_DECIMALS` | `utils::USDC_DECIMALS` | Same value, 6. |
| `earn::lending::plan::key` | removed | `account::ActionKind::from_wire(kind).map(account::ActionKind::signers)` gives the same `Signers`; each action type also has a `KEY` constant (`earn::lending::BorrowLend::KEY`), and a built plan carries it in `key`. |
| `earn::staking::plan::key` | removed | As `earn::lending::plan::key`. |
| `earn::vaults::plan::key` | removed | As `earn::lending::plan::key`. |

#### `live`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `live::account::AssetData`, `live::AssetData` | `account::ActiveAssetData` | It was another name for the same type. |
| `live::account::Clearinghouse`, `live::Clearinghouse` | `account::ClearinghouseState` | It was another name for the same type. |
| `live::account::HistoricalOrder`, `live::HistoricalOrder` | `account::OrderRecord` | It was another name for the same type. |
| `live::account::Margin`, `live::Margin` | `account::holdings::MarginMode` | It was another name for the same type. |

#### `markets`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `markets::reads::L2Book`, `markets::L2Book` (type alias) | `markets::Book` | The alias pointed at this type. |
| `markets::reads::MarketTrade`, `markets::MarketTrade` (type alias) | `markets::Trade` | The alias pointed at this type. |
| `markets::reads::l2_book` | `markets::book::l2_book` | Same arguments; returns `markets::Book`. |
| `markets::reads::recent_trades` | `markets::trades::recent_trades` | Same arguments; returns `Vec<markets::Trade>`. |
| `markets::reads::all_mids` | `markets::mids::all_mids` | `on: Option<&str>` becomes `dex: Dex<'_>`. |
| `markets::reads::all_mids_request` | `markets::mids::all_mids_request` | `on: Option<&str>` becomes `dex: Dex<'_>`. |
| `markets::dexes::dex::DexMeta`, `markets::dexes::DexMeta` | `markets::perps::Meta` | Same fields. `perps`, `find` and `get` take the dex's place first, as before. |
| `markets::dexes::dex::DexAsset`, `markets::dexes::DexAsset` | `markets::perps::PerpAsset` | Same fields. |
| `markets::dexes::dex::DexCtx`, `markets::dexes::DexCtx` (type alias) | `markets::perps::PerpCtx` | The alias pointed at this type. |
| `markets::dexes::dex::DexMetaAndAssetCtxs`, `markets::dexes::DexMetaAndAssetCtxs` | `markets::perps::MetaAndAssetCtxs` | Same fields; `find` takes the place first, as before. |
| `markets::dexes::dex::dex_meta`, `markets::dexes::dex_meta` | `markets::perps::meta` | `dex_meta(t, "xyz")` becomes `meta(t, Dex::Hip3("xyz"))`; still returns `Option`. |
| `markets::dexes::dex::dex_meta_request`, `markets::dexes::dex_meta_request` | `markets::perps::meta_request` | Takes `Dex::Hip3(name)`. |
| `markets::dexes::dex::dex_meta_and_asset_ctxs`, `markets::dexes::dex_meta_and_asset_ctxs` | `markets::perps::meta_and_asset_ctxs` | Takes `Dex::Hip3(name)`; returns `Option<MetaAndAssetCtxs>`. |
| `markets::dexes::dex::dex_meta_and_asset_ctxs_request`, `markets::dexes::dex_meta_and_asset_ctxs_request` | `markets::perps::meta_and_asset_ctxs_request` | Takes `Dex::Hip3(name)`. |
| `markets::dexes::ids` (module) | removed | `Dex`, `coin` and `split_coin` are in `markets::dex`. |
| `markets::dexes::ids::asset_parts`, `markets::dexes::asset_parts` | method `utils::Asset::hip3_parts` | `asset.hip3_parts()`. |
| `markets::perps::rules` (module) | removed | |
| `markets::perps::rules::Rules`, `markets::perps::Rules` | removed | Read the same from `PerpAsset`: `grid()`, `max_leverage`, `allows_cross()` (false means isolated only), a size step of one unit at `sz_decimals` places, and `utils::MINIMUM_ORDER_VALUE` (10). |
| `markets::outcomes::coin::MIN_PRICE`, `markets::outcomes::MIN_PRICE` | `markets::outcomes::Px::MIN_QUOTE` | A `Px`, 0.00001; `.to_dec()` gives the `Dec`. |
| `markets::outcomes::coin::MAX_PRICE`, `markets::outcomes::MAX_PRICE` | `markets::outcomes::Px::MAX_QUOTE` | A `Px`, 0.99999; `.to_dec()` gives the `Dec`. |
| `markets::outcomes::coin::outcome_price`, `markets::outcomes::outcome_price` | removed | `Px::on_grid(price, side.price_rounding())`, then refuse a price where `is_quotable()` is false. |
| `markets::outcomes::coin::outcome_size`, `markets::outcomes::outcome_size` | removed | `Qty::from_dec(size.round(0, Round::Down))`, then refuse `Qty::ZERO`. |
| `markets::outcomes::book::book` | removed | `markets::outcomes::Book::try_from(markets::book::l2_book(t, &coin.to_string(), Precision::FULL).await?)`. |
| `markets::outcomes::book::book_request`, `markets::outcomes::book_request` | removed | `markets::book::l2_book_request(&coin.to_string(), Precision::FULL)`. |
| `markets::outcomes::px::MIN_NOTIONAL`, `markets::outcomes::MIN_NOTIONAL` | removed | Value was 1.5. |
| `markets::outcomes::px::min_shares`, `markets::outcomes::min_shares` | removed | `Qty::least_at(price)` gives the fewest contracts worth the venue minimum of 10 at a price; it is a different rule from the old 1.5 at the short side. |
| `markets::outcomes::template::DeployerLimits`, `markets::outcomes::DeployerLimits` | `markets::outcomes::Remaining` | `deployer_limits` now returns `Remaining { daily, active }`. `active_used` is gone: count the venue's `outcomeMeta` outcomes yourself. |
| `markets::outcomes::template::Pattern`, `markets::outcomes::Pattern` | `markets::outcomes::OutcomeTemplate` | Read `id`, `name`, `role` and `keywords` from the template directly. |
| `markets::outcomes::template::Patterns`, `markets::outcomes::Patterns` | removed | It was a `BTreeMap` of template id to `Pattern`. |
| `markets::outcomes::template::patterns`, `markets::outcomes::patterns` | removed | |
| `markets::outcomes::meta::SubDeployer` | removed | Removed with `Deployer::keys`. `Deployer::sub_deployers` lists each operation with the addresses granted it. |
| `markets::outcomes::market::Catalog`, `markets::outcomes::Catalog` | removed | The `market` module is gone. Read `OutcomeMeta` directly; write titles and side names with `markets::outcomes::fill` and `markets::outcomes::label`. |
| `markets::outcomes::market::Details`, `markets::outcomes::Details` | removed | |
| `markets::outcomes::market::Pair`, `markets::outcomes::Pair` | removed | |
| `markets::outcomes::market::Sides`, `markets::outcomes::Sides` | removed | |
| `markets::outcomes::market::Market` | removed | |
| `markets::outcomes::market::Question` | removed | `markets::outcomes::Question` in 0.1 is the `outcomeMeta` row, not this type. |
| `markets::outcomes::market::Status` | removed | It was `Open` or `Expired` by the deadline; `utils::time::Stamp::passed(now_ms)` gives the same. |
| `markets::outcomes::market::contains`, `markets::outcomes::contains` | removed | It was a case-insensitive ASCII substring test. |
| `markets::outcomes::quote` (module) | removed | `markets::outcomes::Book::top()` gives the best bid and ask. |
| `markets::outcomes::quote::Quality`, `markets::outcomes::Quality` | removed | |
| `markets::outcomes::quote::assess`, `markets::outcomes::assess` | removed | |
| `markets::outcomes::quote::depth_within`, `markets::outcomes::depth_within` | removed | |
| `markets::outcomes::quote::DEPTH_WINDOW` | removed | Value was 0.02. |
| `markets::outcomes::quote::MIN_DEPTH` | removed | Value was 100. |
| `markets::outcomes::quote::WIDE_SPREAD` | removed | Value was 0.02. |
| `markets::outcomes::scan` (module) | removed | |
| `markets::outcomes::scan::CREDIBLE_SPREAD`, `markets::outcomes::CREDIBLE_SPREAD` | removed | Value was 10000 ticks, 0.1. |
| `markets::outcomes::scan::Touch`, `markets::outcomes::Touch` | removed | |
| `markets::outcomes::scan::Edge` | removed | |
| `markets::outcomes::scan::Leg` | removed | |
| `markets::outcomes::scan::Set` | removed | |
| `markets::outcomes::scan::Shape` | removed | |
| `markets::outcomes::scan::Way` | removed | |
| `markets::outcomes::scan::executable`, `markets::outcomes::executable` | removed | |
| `markets::outcomes::scan::exhaustive` | removed | |
| `markets::outcomes::scan::gaps`, `markets::outcomes::gaps` | removed | |
| `markets::outcomes::scan::sets`, `markets::outcomes::sets` | removed | |

#### `trade`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `trade::builder::SignedBy`, `trade::SignedBy` | `account::Key` | `SignedBy::Account` is `Key::Main`, `SignedBy::Agent` is `Key::Agent`. |
| `trade::builder_fee` (module) | `trade::builder::fee` | |
| `trade::leverage` (module) | `trade::margin::leverage` | `UpdateIsolatedMargin` is in `trade::margin::isolated`; both are re-exported at `trade::`. |
| `trade::modify` (module) | `trade::manage::modify` | |
| `trade::order` (module) | `trade::orders` | `Builder` and `OverCap` are in `trade::builder::fee`; all are re-exported at `trade::`. |
| `trade::dexes::update_leverage` | `trade::margin::update_leverage` | Takes `&PerpAsset` instead of `&DexAsset`; same fields. |
| `trade::dexes::update_leverage_for` | `trade::margin::update_leverage_for` | Takes `&PerpAsset` instead of `&DexAsset`. |
| `trade::outcomes::fee::UserFees` | `account::UserFees` | Fields renamed: `spot_cross` is `user_spot_cross_rate`, `spot_add` is `user_spot_add_rate`, `referral_discount` is `active_referral_discount`. The methods became functions: `fees.rate(..)` is `trade::outcomes::rate(&fees, ..)`, `fees.fee(..)` is `trade::outcomes::billed(&fees, ..)`, `fees.fee_of(..)` is `trade::outcomes::billed_for(&fees, ..)`. |
| `trade::outcomes::fee::user_fees` | `account::history::user_fees` | Returns `account::UserFees`. |
| `trade::outcomes::fee::user_fees_request` | `account::history::user_fees_request` | |

#### `transfer`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `transfer::plan::USD_DECIMALS`, `transfer::USD_DECIMALS` | `utils::USDC_DECIMALS` | Same value, 6. |
| `transfer::amount::cut`, `transfer::cut` | removed | It was `value.round(places, Round::Down).to_wire()`. `transfer::Amount::cut(value, places)` still exists and returns an `Amount`, refusing zero and negative figures. |
| `transfer::token::hype`, `transfer::hype` | method `client::Network::hype` | `hype(network)` becomes `network.hype()`. |
| `transfer::keeping::Keeping` | `account::Keeping` | `Keeping::of(word)` becomes `Keeping::of(Some(&Abstraction::parse(word)), None)`. `Pooled` is now `Unified` or `Portfolio` (test with `pools()`), and `dexAbstraction` is `Native` instead of `Apart`. |
| `transfer::link` (module) | `transfer::evm` | `Linked` is re-exported at `transfer::`. |
| `transfer::system` (module) | `transfer::evm` | `HYPE_SYSTEM`, `system_address`, `is_system_address`, `system_token`; all re-exported at `transfer::`. |
| `transfer::arbitrum` (module) | `transfer::bridge` | Calldata, addresses and the minimum deposit are unchanged. The RPC reads now build `client::evm::Query` values run with `client::evm::Rpc::run`. |
| `transfer::arbitrum::ETH_DECIMALS` | `client::evm::NATIVE_DECIMALS` | Same value, 18. |
| `transfer::arbitrum::chain_id` | method `client::Network::arbitrum_chain_id` | `chain_id(network)` becomes `network.arbitrum_chain_id()`. |
| `transfer::arbitrum::RpcError` | `client::evm::RpcError` | Variants differ: `Transport`, `Node { .. }`, `Unreadable { method, detail }`. |
| `transfer::arbitrum::usdc_balance_request` | `transfer::bridge::usdc_balance_request` | Returns `Query<Vec<u8>>` instead of a JSON string; read the answer with `transfer::bridge::usdc_amount`. |
| `transfer::arbitrum::eth_balance_request` | `transfer::bridge::eth_balance_request` | Returns `Query<U256>`; read the answer with `transfer::bridge::eth_amount`. |
| `transfer::arbitrum::balance_of_calldata` | `transfer::erc20::balance_of` | Returns the calldata as bytes (`Vec<u8>`) instead of a `0x` string. |
| `transfer::arbitrum::eth_call_request` | `client::evm::eth::call` | `eth_call_request(to, data)` becomes `eth::call(&CallRequest::to(to, data), BlockTag::Latest)`, a `Query<Vec<u8>>`. |
| `transfer::arbitrum::read_quantity` | removed | `Rpc::run` decodes the answer; `transfer::bridge::usdc_amount` and `eth_amount` scale it. |
| `transfer::arbitrum::quantity` | removed | As `read_quantity`. |

#### `utils`

| 0.0.8 | 0.1 | Notes |
|---|---|---|
| `utils::time::At` | removed | `utils::time::Stamp` remains, at minute precision. |
| `utils::time::BadAt` | removed | |
| `utils::time::DAY_MS` | removed | Value was 86400000 (`i64`). |
| `utils::time::civil` | removed | |
| `utils::time::month_name` | removed | |

### Same path, changed shape

These items keep their path from 0.0.8-beta.1, but their signature, fields or variants changed. A struct whose fields are all public gains fields, which breaks struct literals that name every field; reading the fields and `..Default::default()` still work.

#### `account`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `account::Beside` | fields `staked`, `vaults`, `lending: Vec<Reserve>`, `health: Option<LendingHealth>`; method `lending_health()` | fields `staked`, `vaults` | Drop `lending` and `health` from literals. Read lending from `earn::lending::UserState` (`supplied`, `borrowed`, `risk`, `factor`). |
| `account::ClearinghouseState` | type alias of `account::clearinghouse::Clearinghouse` | struct with the same six fields: `margin_summary`, `cross_margin_summary`, `cross_maintenance_margin_used`, `withdrawable`, `asset_positions`, `time` | No change: the fields are the ones the alias already had. |
| `account::Keeping` | variants `Apart`, `Native`, `Unified`, `Portfolio`; methods `draws(Move)`, `refuses(Move)` | adds variant `Unknown`; `draws` and `refuses` removed | Add an arm for `Keeping::Unknown`. `Keeping::of` now returns `Unknown` for no mode read or a mode it does not know (was `Apart`). The transfer plans refuse a perps-spot move on a pooled account with `transfer::Refused::OneBalance`. |
| `account::Source` | method `words()` | no `words()` | Write the text yourself from the variant. |
| `account::Standing` | `dex(&self, dex: &str) -> Option<&PerpState>` | `dex(&self, dex: &str) -> Option<&ClearinghouseState>` | No change: `PerpState` was an alias of the same type. |
| `account::readiness` | `readiness(perp: &PerpState, mode: &Abstraction)` | `readiness(perp: &ClearinghouseState, mode: &Abstraction)` | No change: same type. |
| `account::standing` | `standing(transport, user, dexes: &[&str])` | `standing(transport, user, dexes: &[Dex<'_>])` | `&["", "xyz"]` becomes `&[Dex::Main, Dex::Hip3("xyz")]`. |

#### `client`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `client::Client` | method `api() -> &Api<H>` | no `api()` | Call `network()`, `http()`, `spent()` on the `Client`, and pass the `Client` itself as the `Transport`. |
| `client::evm::Action` (`client::evm::corewriter::Action`) | method `describe(&self, names: &Names) -> String` | no `describe` | Removed with `client::evm::words`; no replacement. |

#### `deploy`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `deploy::perp_deploy_auction` (`deploy::dexes::perp_deploy_auction`) | returns `DeployAuction` | returns `GasAuction` | No change: `DeployAuction` was another name for `GasAuction`. Rename the type where you name it. |
| `deploy::outcomes::DeployError` (`deploy::outcomes::actions::DeployError`) | variants include `NoSettlements`, `Winners(usize)` | `NoSettlements` and `Winners` removed; adds `NotListed(u32)`, `NotTwoSided(u32)`, `Fallback { outcome, question }`, `NoQuestion(u32)`, `NotOpen { outcome, question }` | Update matches. The new variants come from the settle constructors, which now read `outcomeMeta`. |
| `deploy::outcomes::OutcomeDeploy::settle_outcome` | `settle_outcome(venue, settlement: Settlement) -> Self` | `settle_outcome(venue, meta: &OutcomeMeta, outcome: u32, fraction: Fraction) -> Result<Self, DeployError>` | Pass the `outcomeMeta` read and the outcome id; the settlement's words are copied from `meta`. |
| `deploy::outcomes::OutcomeDeploy::settle_question` | `settle_question(venue, question, settlements: Vec<Settlement>, name, description) -> Result<Self, DeployError>` | `settle_question(venue, meta: &OutcomeMeta, question: u32, winner: u32) -> Result<Self, DeployError>` | Pass the `outcomeMeta` read and the winning outcome id; every named outcome left is settled, the winner at 1 and the rest at 0. |

#### `markets`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `markets::book::Book` | methods `bids()`, `asks()` | no such methods | Use the public fields: `&book.bids`, `&book.asks`. |
| `markets::dexes::Dexes` (`markets::dexes::listing::Dexes`) | `new(PerpDexs, Vec<DexMeta>)`; `dexes`, `dex`, `find`, `get` return `DexMeta` and `DexAsset` | same methods with `Meta` and `PerpAsset` | Rename the types; the arguments are unchanged. |
| `markets::dexes::all_perp_metas` | returns `Vec<DexMeta>` | returns `Vec<Meta>` | Rename the type. |
| `markets::dexes::perp_dex_limits` | `(transport, dex: &str)` | `(transport, dex: Dex<'_>)` | `"xyz"` becomes `Dex::Hip3("xyz")`. |
| `markets::dexes::perp_dex_limits_request` | `(dex: &str)` | `(dex: Dex<'_>)` | As `perp_dex_limits`. |
| `markets::dexes::perp_dex_status` | `(transport, dex: &str)` | `(transport, dex: Dex<'_>)` | As `perp_dex_limits`. |
| `markets::dexes::perp_dex_status_request` | `(dex: &str)` | `(dex: Dex<'_>)` | As `perp_dex_limits`. |
| `markets::outcomes::Coin` (`markets::outcomes::coin::Coin`) | `market(side, size: Dec)`; `limit(side, price: Dec, size: Dec, tif)`; `lookup(text)`; `matches(needle)` | `market(side, size: Qty)`; `limit`, `lookup`, `matches` removed | `coin.market(side, Qty::from_dec(size)?)`, refused at zero. For `limit`, put the price on the grid and call `coin.order(side, px, qty, tif)` with a `Px` and a `Qty`. For `lookup`, add the `#` and call `Coin::parse`. |
| `markets::outcomes::Deployer` (`markets::outcomes::meta::Deployer`) | method `keys() -> Vec<SubDeployer>` | no `keys()` | Read `sub_deployers`, a list of each operation with the addresses granted it. |
| `markets::outcomes::Family` (`markets::outcomes::template::Family`) | method `slug()` | method `as_str()` | Rename the call; it returns the same text. `Display` and `FromStr` use it as before. |
| `markets::outcomes::OutcomeMeta` (`markets::outcomes::meta::OutcomeMeta`) | method `venue_of(who: &str) -> Option<&str>` | no `venue_of` | Search `meta.deployers` for the address or venue name; `meta.deployer(venue)` finds a deployer by venue. |
| `markets::outcomes::deployer_limits` (`markets::outcomes::template::deployer_limits`) | `(transport, venue: &str, live: u32) -> DeployerLimits` | `(transport, venue: &str) -> Remaining` | Drop `live`. `daily_remaining` is `daily`, `active_remaining` is `active`; count the live outcomes yourself. |
| `markets::perps::Meta` (`markets::perps::meta::Meta`) | `perps()`, `find(name)`, `get(asset)` | `perps(place)`, `find(place, name)`, `get(place, asset)` | Pass the dex's place first: 0 for the main dex. |
| `markets::perps::MetaAndAssetCtxs` | `ctx(asset)`, `find(name)` | `ctx(place, asset)`, `find(place, name)` | Pass the place first: 0 for the main dex. |
| `markets::perps::PerpAsset` | fields without `growth_mode` and `deployer_fee_scale` | adds `growth_mode: Option<String>` and `deployer_fee_scale: Option<Dec>` | Add both fields to struct literals (`None` for a main-dex perp). |
| `markets::perps::meta` | `meta(transport) -> Meta` | `meta(transport, dex: Dex<'_>) -> Option<Meta>` | `meta(t, Dex::Main)` and handle `None`, which is a dex the venue does not know. |
| `markets::perps::meta_request` | `meta_request()` | `meta_request(dex: Dex<'_>)` | `meta_request(Dex::Main)`. |
| `markets::perps::meta_and_asset_ctxs` | `(transport) -> MetaAndAssetCtxs` | `(transport, dex: Dex<'_>) -> Option<MetaAndAssetCtxs>` | `meta_and_asset_ctxs(t, Dex::Main)` and handle `None`. |
| `markets::perps::meta_and_asset_ctxs_request` | `()` | `(dex: Dex<'_>)` | `meta_and_asset_ctxs_request(Dex::Main)`. |
| `markets::spot::SpotToken` (`markets::spot::meta::SpotToken`) | fields without `deployer_label` | adds `deployer_label: Option<String>` | Add the field to struct literals. |

#### `trade`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `trade::BuilderSetting::approval` | `approval(order, approved, by: SignedBy)` | `approval(order, approved, by: account::Key)` | `SignedBy::Account` is `Key::Main`, `SignedBy::Agent` is `Key::Agent`. |

#### `transfer`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `transfer::key` | returns `Option<Key>` | returns `Option<Signers>` | No change: `transfer::Key` was another name for `account::Signers`. Rename the type where you name it. |

#### `utils`

| Item | 0.0.8 | 0.1 | What to write |
|---|---|---|---|
| `utils::Grid` (`utils::rounding::Grid`) | `limit(asset, side, price, size, tif)`, `market(asset, side, reference, slippage, size)` | removed | `Order::limit(asset, side, grid.price(price, side)?, grid.size(size)?, tif)` and `Order::market(asset, side, grid.market_bound(reference, slippage, side)?, grid.size(size)?)`, with `trade::Order`. |

## If you also use df-papertrade-core

df-papertrade-core 0.0.13-beta is built on 0.1.1. Its own changes since 0.0.8-beta.1 are in its [CHANGELOG](https://github.com/dennisfurrer/df-papertrade-core/blob/main/CHANGELOG.md). The ones met while moving apps to it: `math::value` takes `Rates` instead of a win fee rate (`Rates::default()` is the old behaviour, `Rates { win_fee, ..Rates::default() }` keeps a fee read from the chain), `open_limits` no longer takes the `TradingState`, `OpenRefusal` gained `LeverageTooLow`, `Intake` no longer has `maximum_net_open_interest` (each instrument has `long_cap` and `short_cap`), and `Send::describe` is gone.
