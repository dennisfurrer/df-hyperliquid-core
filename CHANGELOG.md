# Changelog

All notable changes to df-hyperliquid-core. Versions follow [semantic versioning](https://semver.org) from 0.1.0.

## 0.2.1

Found while moving df-elysium-core, df-papertrade-core and hip4-tui to 0.2.

Added:

- `Order::new(asset, side, price, size, order_type)`: an order from an `OrderType` already chosen, so code that keeps an order's type can rebuild it without matching on `OrderType`.
- `BuilderFee::allowed_on(kind)`: `Err(OverCap)` when a fee is over the builder cap of an order of `kind`, so callers no longer build an `OverCap` themselves.

Changed:

- `client::evm::Fees` and `client::evm::NodeError` are no longer `#[non_exhaustive]`: callers build both, the fees for a transaction and a JSON-RPC error, whose fields the JSON-RPC spec fixes. Struct literals work again.

## 0.2.0

Public enums and the structs the crate returns are `#[non_exhaustive]`, so later 0.2.x releases can add a variant or a field without a breaking change. No behaviour changes and nothing is removed. [MIGRATION.md](MIGRATION.md#from-01-to-02) lists what to change and which types stay exhaustive.

Changed:

- 143 enums are `#[non_exhaustive]`: a `match` on them outside the crate needs a `_` arm. `Side`, `OutcomeSide`, `Round`, `Json`, `Dex`, `Network`, `Readiness`, `OrderLookup`, `ExchangeResponse` and `Status` are not, since they cannot gain a variant.
- 185 structs with public fields are `#[non_exhaustive]`: answers, frames, decoded reads, plans and errors. Outside the crate they are read, not built; build one in a test from the venue's JSON with serde. The 39 structs callers build (plan contexts, order intents, transport, HyperEVM calls, deploy and validator payloads, signing domains, socket settings) keep their struct literals.

## 0.1.1

Every L1 action checked on testnet, and the units of irreversible deploy fields confirmed on chain.

Added:

- `just testnet` (`tests/testnet`): signs every L1 action in `vectors/` with a new key that has no account, posts it to testnet, and requires the node to recover that key's address. 159 actions pass, including every action that was built from the docs alone: `trailingStop`, `cDeposit2`, `setTokenAnnotation`, `setDeployerLabel`, `setFundingClamps`, `setDeployerFees`, the `normal` margin mode and a removed open interest cap.
- `Template::parse`, which returns `VersionTooLarge` for a template version past 255.
- Doc comments that name the unit of every irreversible deploy field, each confirmed against mainnet or testnet deploys and the state they produced: `commission_bps` is basis points (500 is 5%), `initial_wei` and every `maxGas` are HYPE wei with 8 decimals, genesis amounts are the token's wei, open interest caps are millionths of the collateral token, and `spotDeployState` balances are whole tokens with only `totalGenesisBalanceWei` exact.

Changed:

- Annotation lengths (`setTokenAnnotation`, `setPerpAnnotation`) are counted in UTF-8 bytes, as the venue counts them: testnet refused a 6-character, 18-byte display name. Text the venue would refuse is now refused before signing.
- HIP-4 keyword errors say bytes, which is what they always counted and what the venue counts.

Deprecated:

- `SpotDeploy::EnableAlignedQuoteToken` and `SpotDeploy::DisableAlignedQuoteToken`: mainnet and testnet refuse both for every shape.
- `Template::classify`, which drops a version past 255 without an error. Use `Template::parse`.

Documented:

- `ValidatorProfile::node_ip` and `ProfileChange::node_ip` must be IPv4: the node refuses an IPv6 address.
- A later `userGenesis` for the same address replaces its amount, and `setDeployerTradingFeeShare` only takes a strictly lower share.

## 0.1.0

The first stable release: full parity with the Hyperliquid API docs, every public item documented, and benchmarks.

Added:

- Info: `perpAnnotation`, `perpConciseAnnotations`, `approvedBuilders`.
- Orders: trailing stop orders.
- Transfers and staking: `sendToEvmWithData`, `hip3LiquidatorTransfer`, `cDeposit2`, `claimRewards`.
- Account: `reserveRequestWeight`, `convertToMultiSigUser`.
- HIP-4 outcomes: `mergeQuestion`, `negateOutcome`.
- Spot deploy: every `spotDeploy` sub-action, the freeze actions and `finalizeEvmContract` (`deploy::spot`).
- HIP-3 deploy: every `perpDeploy` sub-action (`deploy::perp`).
- Validator actions: `validatorL1Stream`, `authorizeAqav2Role` (`deploy::aligned`), `CValidatorAction` and `CSignerAction` (`earn::validator`).
- Websocket: the `l2`, `pac` and `sac` channels, and `L2Diff::apply` to keep a book from `l2` diffs.
- HyperEVM JSON-RPC: `net_version`, `web3_clientVersion`, `eth_syncing`, block reads by hash, block receipts, transaction counts, transactions by block and index, and system transactions.
- Conformance vectors from @nktkas/hyperliquid 0.33.3 and hyperliquid-python-sdk 0.24.0 for every new signed action.
- Benchmarks against hypersdk and the official Python SDK in `bench/`.
- Doc comments on every public item, enforced with the `missing_docs` lint.

Not included: `userDexAbstraction` and `agentEnableDexAbstraction` (deprecated, no longer accepted by mainnet) and the testnet-only HIP-3 `star` operations.

## 0.0.12-beta

- Crate description names HIP-4 deploys instead of deploy as a whole.
- Examples: signed order, websocket subscription, HIP-4 order and a wasm build.

## 0.0.11-beta

- License changed from BUSL-1.1 to MIT OR Apache-2.0.
- The crate links to its repository and docs.rs.

## 0.0.10-beta and earlier

Published under BUSL-1.1. See the [GitHub releases](https://github.com/dennisfurrer/df-hyperliquid-core/releases).
