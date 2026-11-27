# Changelog

All notable changes to df-hyperliquid-core. Versions follow [semantic versioning](https://semver.org) from 0.1.0.

## 0.2.5

No code changes to the crate. Benchmark harness in `bench/`:

- Each results file records the power source and energy mode (`pmset` on macOS, `/sys/class/power_supply` and the ACPI platform profile on Linux). The runner warns when it runs on battery or in low power mode, the `.md` and `.pdf` show the warning, and `just bench-share` repeats it.
- On macOS the benchmark programs ask for the user-interactive quality of service on every thread that does timed work, so the scheduler keeps them on performance cores more reliably. The Python SDK's main thread does the same.
- `just bench-share` says "df-hyperliquid-core only, no comparison" for a run without competitors, and "no hypersdk comparison" for a run without the hypersdk row.

## 0.2.4

No code changes. The README's benchmarks are from the reproducible harness in `bench/` (`just bench-compare`, offline, no key needed), at the tag `bench-2026-11-27`.

## 0.2.3

Added:

- Feature `secp256k1`: on native targets, signing and recovery go through libsecp256k1 (the C library Bitcoin Core uses) instead of k256. Signing takes about half the time and recovery about a fifth, with the same bytes: both follow RFC 6979 and give low-s signatures, and recovery refuses the same signatures (a high `s`, a bad `v`, an `r` or `s` out of range). Tests and a fuzz target compare the two backends. Each key gets its own libsecp256k1 signing context, blinded with 32 bytes from the operating system's random source, and the key is wiped on drop. The build needs a C compiler for the target. Off by default; no effect on wasm.
- `Signer::signature_is_known_good`, `false` by default. When `true`, `Client::send` and `Client::send_user` do not recover the signature before posting it in a release build. `LocalKey` returns `true`, since it signs with the key its address comes from; a wallet, remote signer or hardware module must keep `false`. With k256 this takes about 80 us of CPU off every order sent with a local key.

Changed:

- `live::decode` reads `l2Book`, `trades` and `bbo` frames straight from the text when they are in the venue's compact layout, and reads every other data frame in one pass instead of two. A frame in any other spelling, and every frame that does not read, goes through the previous two-pass reader, so results and error messages are unchanged. Tests check that the readers give the same frame for every recorded frame, every cut of one, a set of changed copies and 5,500 mainnet frames in `bench/data`, and a fuzz target checks that `decode` gives the same answer as the two-pass reader for any text. On the recorded mainnet corpus, about 4.3 times as many frames a second as 0.2.2, with a third of the allocations on `l2Book`.
- `Dec::parse` reads up to nineteen digits in a `u64` before falling back to the `i128` reader, and `Address` reads hex through a lookup table. Same results.
- `Subscription::L2Book` documents how often the venue sends a book: without `fast`, BTC came about every 7 s on mainnet in November 2026, though the venue's docs say at most every 0.5 s.

## 0.2.2

Changed:

- `client::Method` (`Get`, `Post`) is no longer `#[non_exhaustive]`. A `Transport` you write has to send every method the crate asks for, so it matches on `Method`, and a `_` arm there could only fail at run time. Found moving Atlas to 0.2.

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
