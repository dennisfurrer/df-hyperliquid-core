# Changelog

All notable changes to df-hyperliquid-core. Versions follow [semantic versioning](https://semver.org) from 0.1.0.

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
