# Changelog

All notable changes to df-hyperliquid-core. Versions follow [semantic versioning](https://semver.org) from 0.1.0.

## 0.2.7

No code changes to the crate.

- The crates.io description and the README state the signing results like with like: the fastest signing of the SDKs benchmarked with the optional `secp256k1` feature, on the same C library go-hyperliquid uses, and a pure-Rust default level with the fastest other Rust SDK (hyperliquid_rust_sdk). The figures are those of 0.2.6's results file, `bench/results/2026-11-28-001007-apple-m5-max`.

Benchmark harness in `bench/`:

- The PDF's signing tiles and `just bench-share` compare like with like: the default build (k256, pure Rust) against the fastest other Rust SDK and the SDKs that do not use C, and the `secp256k1` build against the SDKs that sign with libsecp256k1 (go-hyperliquid built with cgo, the Python SDK with coincurve). Before, both builds were compared against the fastest signer of any kind, which put a pure-Rust build next to a C library.

## 0.2.6

Added:

- `utils::sign::Breakdown`: every stage of an L1 signature in the order it is built, from the same encoders `L1Action::digest` and `L1Action::body` use: the msgpack bytes, the action hash over them with the nonce, the vault and an optional `expiresAfter`, the EIP-712 payload a wallet is asked to sign, and the digest. `Breakdown::of` and `Breakdown::expiring` build it; with the `sign` feature, `recover(&signature)` gives the address a signature recovers to over that digest. Useful when the venue answers "User or API Wallet 0x... does not exist": that answer means the address it recovered is not one it knows, and comparing these stages with another SDK's, or the recovered address with the key's, shows where they differ.
- `utils::sign::agent_digest(connection_id, network)`: the phantom agent's digest under `Domain::EXCHANGE`.
- Examples: `quickstart` (what is listed, every mid, BTC's book and candles, and with `HL_ADDRESS` an account's state; no key) and `market_maker` (two post-only quotes on BTC from the live touch, requoted as one `batchModify` over the websocket, with a position and loss limit, the dead man's switch, cancel on exit, a watchdog, and the time each stage of a requote takes). By default the market maker reads mainnet and signs every requote for testnet without sending it; `HL_SEND=1` trades on testnet. When the venue answers "does not exist" it prints the likely causes, the nonce, vault, expiry and network it signed with, each `Breakdown` stage and the recovered address next to the key's, never the key. `packages/core/examples/README.md` describes every example.

Changed:

- Signing an L1 action hashes the EIP-712 domain separator, the agent type hash and the source string from constants instead of for each signature, so the digest takes two keccak blocks instead of eight: about 1.8 us to 0.75 us on an Apple M5 Max, and about 1 us off each signature with either backend. The bytes signed are the same; tests hold each constant to the value it replaces. Nothing in k256 or libsecp256k1 changed.
- The workspace lint for `unsafe_code` is `deny` instead of `forbid`, so the market maker's macOS thread priority module can allow it. The library still forbids it, in `lib.rs`.

Benchmark harness in `bench/`:

- `just bench-compare` also measures hyperliquid-sdk-rs 0.1.2, quicknode-hyperliquid-sdk 0.1.9, hyperliquid_rust_sdk 0.6.0, @nktkas/hyperliquid 0.33.3 (Node) and go-hyperliquid 0.47.3, each pinned by its own lockfile. The TypeScript and Go rows are skipped with a note when Node 22.12 and pnpm, or Go, are missing, and the QuickNode rows when `protoc` is missing. `just bench-doctor` lists node, pnpm, go and protoc.
- `--profile release|release-lto|both` builds the Rust rows with Cargo's `release` profile, with a `release-lto` profile (fat LTO, one codegen unit), or both, and reports each.
- Each results `.md` starts with one table that has one row per SDK and build, numbers only: signing median and p99, build + sign + body, parse rate, parse p99, allocations per frame, the two round trips and the live frame age.
- A row whose signature differs from df-hyperliquid-core's no longer stops the run: it is timed and named as failing in the signing table. A competitor whose program fails is noted under Skipped.
- `just bench-share` gives df-hyperliquid-core's speed divided by each competitor's, for parsing and for signing, in at most 280 characters; a figure below 1 means that competitor was faster. The PDF's headline tiles compare against the fastest other SDK on each test instead of hypersdk.

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
