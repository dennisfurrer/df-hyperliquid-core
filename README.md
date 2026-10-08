# hl-rust

The Hyperliquid family. `hl-core` is the backbone; every other crate is one product area on top of it.

```mermaid
flowchart BT
  CORE("<b>hl-core</b><br/>vocabulary, Dec, asset ids,<br/>wire, signing, nonces,<br/>client, weights")
  TRADE("<b>hl-trade</b><br/>order, cancel, modify, TIF,<br/>ticket, builder fees, reads")

  ACCOUNT("hl-account") --> CORE
  TRANSFER("hl-transfer") --> CORE
  LEND("hl-lend") --> CORE
  STAKE("hl-stake") --> CORE
  VAULT("hl-vault") --> CORE
  HYDRO("hl-hydromancer") --> CORE
  WS("hl-ws") --> CORE
  EVM("hl-evm") --> CORE
  TRADE --> CORE

  PERPS("hl-perps") --> TRADE
  SPOT("hl-spot") --> TRADE
  HIP3("hl-hip3") --> TRADE
  HIP4("hl-hip4") --> TRADE

  STORE("<b>hl-store</b><br/>interests, folded state,<br/>views a page reads") --> WS
  STORE --> ACCOUNT
  STORE --> PERPS
  STORE --> SPOT
  STORE --> HIP3

  ANALYTICS("<b>hl-analytics</b><br/>builders, deployers,<br/>revenue, payouts") --> HIP3
  ANALYTICS --> HIP4
  ANALYTICS --> HYDRO
  ANALYTICS --> ACCOUNT
```

| Crate | Owns |
|---|---|
| `hl-core` | vocabulary: exact decimals (`Dec`; literals checked when built, checked sums), the side of an order, a fill or a book (`Side`, `B`/`A`), asset ids (perps, spot, HIP-3, HIP-4) and their builder caps, the perp dex (`Dex`) and HIP-3 coins (`dex:SYMBOL`, an id back to its dex and index), builder fees in tenths of a basis point, exact integer units (USDC 1e6, HYPE 1e8), floats kept as written (`Figure`); each network's endpoints, chains (signature, Arbitrum, HyperEVM) and its USDC and HYPE; system addresses; who may sign what (every action kind, main key or agent, `may_sign`); the deploy auction (`GasAuction`); msgpack wire and JSON in written order; L1 and user-signed signing; nonces; the info and exchange client over a transport trait (`/info` bodies, `/exchange` answers with their `data`) behind the rate-limit gate |
| `hl-account` | clearinghouse and spot state, account modes and changing them (`userSetAbstraction`, `agentSetAbstraction`; unified back to standard is refused), usable balances (what each kind of market can draw on, read the way the mode says, and which balance pays), worth (counted once, perps not twice) and holdings beside the balances, onboarding (what is left before an account can trade, from live state), readiness (whether a builder can be paid and what an order naming one needs first), limits (`userRateLimit`: what an address may still send and how fast that runs down), signer rules (hl-core's, re-exported), agent wallets (`approveAgent`, `extraAgents`, `userRole`; no revoke, never re-approve an old agent), builder fee approval (`approveBuilderFee`, `maxBuilderFee`), subaccounts (`createSubAccount`, `subAccountTransfer`, `subAccountSpotTransfer`, `subAccounts`), referrals (`setReferrer`, `referral`), `userFees`, `portfolio` |
| `hl-transfer` | usdSend, spotSend, sendAsset (also between the account's own balances), `agentSendAsset` (the same, signed by an agent), usdClassTransfer, withdraw3, amount checks, preTransferCheck, system-address guard, Core to HyperEVM (`spotSend` from a separate account, `sendAsset` from a pooled one; the way back is hl-evm's), plans (the key each needs, the balance a read back checks), ledger history and the row a send made, Arbitrum bridge deposit calldata on both networks |
| `hl-trade` | `order` (limit and trigger, grouping, builder and its cap), `cancel`, `cancelByCloid`, `modify` and `batchModify` (always place for a trigger), `scheduleCancel` (the dead man's switch), `noop`, `twapOrder`, `twapCancel`, `updateLeverage`, `updateIsolatedMargin`; margin tables and the leverage a perp allows (shared by hl-perps and hl-hip3); tick and lot rounding (`Grid`: a price never rounds against the trader, a size toward zero); the ticket (intent to the exact action or a typed refusal: minimum order value, market orders `FrontendMarket` at a slippage bound and never `Gtc`, TP/SL grouped and modified, scales, closing a position); builder fees as a setting (a share of each kind's cap in integer math, no builder named) and the `approveBuilderFee` check; what `/exchange` answers per order, cancel and modify; `openOrders`, `frontendOpenOrders`, `orderStatus`, `historicalOrders`, `userFills`, `userFillsByTime`, `userTwapSliceFills`, `userFees`, `l2Book`, `candleSnapshot`, `recentTrades`, `allMids` |
| `hl-perps` | main dex `meta` and `metaAndAssetCtxs`, each perp's asset id, grid and rules, margin tables, leverage checked against what the asset allows and the tier a position reaches, isolated margin; positions and margin summaries (`clearinghouseState`), `activeAssetData`, `fundingHistory`, `userFunding` (windows and pages), `predictedFundings`, `perpsAtOpenInterestCap`; the account reads take a HIP-3 dex |
| `hl-spot` | `spotMeta` and `spotMetaAndAssetCtxs`, token lookup (index, name, token id, `NAME:0x<tokenId>`, size and wei decimals), pair lookup (index, book name, asset id, tokens) and its grid, a token's USDC price, `tokenDetails`, `spotDeployState`, `spotPairDeployAuctionStatus`; nothing keyed to one network's indexes |
| `hl-hip3` | `perpDexs` and `allPerpMetas` lined up, per-dex `meta` and `metaAndAssetCtxs`, dex asset ids and coins, `perpDexLimits`, `perpDexStatus`, funding multipliers, rates and clamps, margin tables and leverage, `perpDeployAuctionStatus`; no `perpDeploy` until a vector pins it |
| `hl-hip4` | outcome coins (`#`/`+` spellings, asset ids), prices as ticks of 0.00001 and sizes as whole contracts, market orders at the edge of the range or within a slippage, the minimums, whether a mid is a probability, `outcomeMeta` read into markets and questions in words (recurring markets, expiry), `outcomeTemplates` and deployer limits, `l2Book`, the complete-set scan (before fees), holdings from the spot side with fills beside them, structural fills kept out of volume, the outcome fee model (closing side only), `settledOutcome`, `userOutcome` split and merge, `outcomeDeploy` (register, settle, sub-deployers; settling cannot be undone) and the checks each step passes first, `activateOutcomeDeployer` |
| `hl-lend` | `borrowLend` (supply, withdraw, borrow, repay; `null` for all), `allBorrowLendReserveStates`, `borrowLendReserveState`, `borrowLendUserState`; utilization, the borrow and supply rates, what a holding backs and what can still be borrowed, health; plans |
| `hl-stake` | `cDeposit`, `cWithdraw`, `tokenDelegate`, validators, delegator summary, delegations, rewards and history, a whole stake with its pending withdrawals; the delegation lock and unstaking queue (a day and seven on mainnet, two and five minutes on testnet, five at once); plans |
| `hl-vault` | `vaultTransfer`, `vaultDetails` (strategies, follower state, what may leave now), `userVaultEquities`, `leadingVaults`; HLP on each network; lockups (HLP four days; others a day on mainnet, twenty seconds on testnet); plans |
| `hl-hydromancer` | Hydromancer, a third-party data provider, mainnet only (`Hydromancer::mainnet`, the caller's key as a bearer header, cut from any error): the L4 book (`l4Book` snapshot decoder, the `l4BookUpdates` socket frames, an `Assembler` that resyncs on a height gap, `Book` with matching and owners, L4 to L2 levels), the outcome builder dashboard, `perpPriceHistoryByTime`, the `builderFills` socket, and Hyperliquid's daily `builder_fills` files |
| `hl-ws` | the public websocket, sans-IO: every subscription the docs list (and `assetCtxs`, `spotAssetCtxs`, `userHistoricalOrders`), the subscribe, unsubscribe, ping and post frames, each channel decoded into exact `Dec` (an unknown one kept as written), and the connection's state machine: resubscribe on open, a ping after 30 s of quiet, a reconnect after 60 s of silence, delays doubling from 250 ms to 20 s with jitter and no last attempt, the venue's per-IP limits refused before anything is sent |
| `hl-evm` | HyperEVM (999 mainnet, 998 testnet) and the Arbitrum side of the bridge: typed JSON-RPC over `Http` (HyperEVM's RPC by network, Arbitrum by URL; `eth_bigBlockGasPrice`, `eth_usingBigBlocks`), exact `U256` quantities, ABI, RLP and EIP-1559 transactions (calls and contract creations; signing is its own `sign` feature), every read precompile `L1Read.sol` names, CoreWriter actions encoded, decoded from `RawAction` logs and read in words, system addresses and the transaction that sends a token back to Core |
| `hl-web` | the wire for the browser and Workers, on their globals and never `window`: hl-core's `Http` through `fetch`, `Date.now` as the clock, and `Feed`, one hl-ws session over `WebSocket` (subscriptions kept while down and sent on every open, a quiet socket pinged and a silent one dropped on `setTimeout`, dialled again after the session's delay with `Math.random` jitter, a post refused while down and an unanswered one reported, never sent again; typed events read with an async `next`) |
| `hl-store` | the live state an app keeps, sans-IO: interests a page registers and holds by a handle, counted into hl-ws subscriptions (subscribe, and unsubscribe once nothing needs one, into a `Session` of the same network only) within the venue's per-IP limits; socket frames and the crates' `/info` reads folded per market (book, prints, candles, mids, contexts) and per account and dex (clearinghouse, spot, mode, resting orders, fills, fundings, ledger), snapshots told from deltas, a fill held once, each piece stamped with when it was last told and last changed; views: positions, resting orders, what an account can use on a market by its mode (hl-account's), worth counted once, the touch, the mid, a market's context; up to ten accounts, a subaccount at its own address with its own mode |
| `hl-analytics` | who builds and deploys, computed from what the caller read (sans-IO, no clock): the builder board (every builder ranked by outcome volume over 24h, 7d, 30d and all, the fills no builder routed beside them holding no place, shares in hundredths of a percent), the curated builder labels with their provenance, a builder fee as numbers (each kind's cap, a share of it, what it pays on a notional), what a builder code has been paid (`referral`); every HIP-3 dex as its deployer set it up (delegates, live and delisted assets, fee scale range, funding settings) and what its assets are doing, the deployers of more than one dex, and the builders that routed perps on a day from Hyperliquid's day files (ranked, with who was quiet and whose read failed) |

## Rules

- **Layering:** a crate depends on `hl-core`. The four market crates may also depend on `hl-trade`. A composite (`hl-store`, `hl-analytics`) may depend on any crate below it, never on the umbrella or a wire. Nothing else crosses.
- **Standalone:** no `hl-*` crate depends on an app. hip4-tui, atlas and coreserve are consumers, never dependencies.
- **Targets:** every crate builds native and `wasm32-unknown-unknown`. No tokio, no OS keychain in the family; transports plug in from outside.
- **Money:** `Dec`, never floats (`clippy::float_arithmetic` denied in every crate).
- **Signing:** golden vectors before any new action (`vectors/`, each file naming the implementation that made it). A crate never holds the user's wallet key.
- **Features:** the curve is `hl-core`'s `sign` feature. Every crate takes `hl-core` without it, so an app that only reads never compiles `k256`; one that signs asks for it.
- **Rate limit:** `hl_core::Api` gates Hyperliquid's `/info` and `/exchange`, one network's endpoints behind one gate: reads hold back near the IP's 1,200 a minute, and a 429 closes the gate for everything. Hydromancer and HyperEVM's RPC go through `Http` directly, outside it.
- **Network:** mainnet or testnet, and every signature names one: an action signed for one network is never valid on the other, so nothing assumes it. `/exchange` is never retried.

## Check

`just check` is the gate before any commit: fmt, clippy native and wasm, every test, and the feature check.
