# Examples

Run every command from the repository root. Nothing signs or sends unless a command says so: the
examples that place orders print the signed request and stop unless `HL_SEND=1` is set.

| Example | What it shows | Command |
|---|---|---|
| `quickstart` | What is listed, every mid, one coin's book and last hour of candles, and an account's value, positions, spot balances and open orders, read by address with no key | `cargo run -p df-hyperliquid-core --release --example quickstart` |
| `subscribe` | The websocket: BTC's book and trades, ten updates | `cargo run -p df-hyperliquid-core --release --example subscribe` |
| `market_order` | The unsigned market order a $20 BTC buy would send, from mainnet's meta, book and context | `cargo run -p df-hyperliquid-core --release --example market_order` |
| `signed_order` | Place and cancel: a signed post-only BTC buy 30% under the mid and the cancel for it; `HL_SEND=1` places and cancels it on mainnet | `HL_PRIVATE_KEY=0x... cargo run -p df-hyperliquid-core --release --example signed_order` |
| `outcome_trade` | A HIP-4 outcome market: a signed post-only buy of Yes at half the bid and its cancel; `HL_SEND=1` sends both | `HL_PRIVATE_KEY=0x... cargo run -p df-hyperliquid-core --release --example outcome_trade` |
| `web_book` | The `web` transport compiled to wasm: `btcMid()` through the global `fetch` | `cargo build -p df-hyperliquid-core --example web_book --no-default-features --features sign,web --target wasm32-unknown-unknown` |
| `market_maker` | A quoting loop: two post-only quotes from the live touch, requoted by `batchModify` over the websocket, with its own timing figures | `cargo run -p df-hyperliquid-core --release --example market_maker` |

`quickstart` takes `HL_COIN` (default `BTC`) and `HL_ADDRESS` (any account). Each file starts with
what it shows, the environment variables it reads and the command that runs it.

## Market making

`market_maker` quotes one perp from the live touch and shows how long the SDK takes per requote.

### What it does

- Follows the coin's `bbo` (or the fast `l2Book` with `HL_BOOK=fast`). Both send the whole touch
  each time, so after a reconnect the next frame is already a full snapshot.
- Quotes two post-only (`Alo`) orders around the mid, half the spread `HL_SPREAD_BPS` each way, with
  the centre moved against the position by up to `HL_SKEW_BPS` at the full position.
- Puts every price on the venue's grid with exact decimals and keeps each quote `HL_BEHIND_TICKS`
  (default 1) behind the best price on its own side, so a quote cannot cross.
  `HL_SPREAD_BPS=0 HL_BEHIND_TICKS=0` quotes at the best bid and ask, still post-only, for a thin
  testnet book. A post-only quote the venue still refuses as crossing (it happens in a fast
  market) is counted as normal and quoted again. So is "Cannot modify canceled or filled order",
  the answer to a modify of a quote that filled while the modify was on its way: the venue
  reports the fill after the modify has left. The next touch places a new quote on that side.
- When the websocket drops it dials again. A request that cannot be sent meanwhile is not held to
  send later; the quotes stay as they were and the next touch after the reconnect requotes them.
  A request sent before the drop whose answer was lost holds requoting until the watchdog cancels
  everything over HTTP.
- Cuts each size to the lot and raises it to the venue's $10 minimum order value when it falls
  under, checked before anything is sent.
- Requotes only when a price moves a tick or more, as one `batchModify` keyed by client order id,
  sent over the websocket's `post` method rather than a new HTTP request.
- Tracks the position and the profit and loss from the account's `userFills`, and drops a quote the
  venue reports filled or cancelled on `orderUpdates`.
- Stops quoting the side that would add to the position at `HL_MAX_POSITION_USD`, and stops and
  cancels everything at a loss of `HL_MAX_LOSS_USD`.

### Running it

| Mode | What happens | Command |
|---|---|---|
| Dry run (default) | Reads the mainnet BTC touch with no account, works out and signs every requote for testnet, sends nothing. Without `HL_PRIVATE_KEY` it signs with a throwaway key made in memory | `cargo run -p df-hyperliquid-core --release --example market_maker` |
| Testnet | Trades on testnet with your key, following the testnet book | `HL_SEND=1 HL_PRIVATE_KEY_FILE=$HOME/.hl/testnet.key cargo run -p df-hyperliquid-core --release --example market_maker` |
| Mainnet | Trades on mainnet; needs both flags | `HL_SEND=1 HL_MAINNET=1 HL_PRIVATE_KEY_FILE=$HOME/.hl/mainnet.key cargo run -p df-hyperliquid-core --release --example market_maker` |

`HL_PRIVATE_KEY_FILE` names a file that holds the key as hex, so the key stays off the command
line and out of the shell's history (`HL_PRIVATE_KEY` takes the key itself). With `HL_SEND=1` the
book feed and the orders are on the same network; the program prints both at the start and
refuses to run if they differ. Use an API wallet's key and set `HL_ADDRESS` to the account it
trades for. `HL_SECS` stops it after
a number of seconds; otherwise Ctrl-C stops it, which cancels the quotes first. Every setting is
listed at the top of `examples/market_maker/main.rs`.

### Where the time goes

Every `HL_REPORT_SECS` and on exit it prints the median and p99 of each stage of a requote:

- **parse frame**: reading the frame's text into the SDK's types, with the SDK's own decoder;
- **quote logic**: working out the two quotes and what changes;
- **build and sign**: building the action, hashing it and signing it, and writing the request body;
- **frame to signed**: the whole, from the frame's arrival to a signed request ready to send;
- **send to answer** (with `HL_SEND=1`): from sending to the venue's answer.

The quoting thread and the runtime's workers ask macOS for the user-interactive QoS class
(`market_maker/cores.rs`, the only `unsafe` in the examples), as the benchmark does, so the
scheduler is much more likely to keep them on performance cores. Elsewhere that call does nothing.

Four 60-second dry runs on an Apple M5 Max, BTC, release builds, `HL_SPREAD_BPS=1
HL_MIN_REQUOTE_MS=100`, median and p99 in microseconds:

| Signer | `HL_SPIN` | Parse frame | Quote logic | Build and sign | Frame to signed | Requotes |
|---|---|---|---|---|---|---|
| default (k256) | off | 1.4 / 3.2 | 6.0 / 9.5 | 114.2 / 165.8 | 125.4 / 180.9 | 57 |
| default (k256) | on | 0.4 / 1.0 | 2.4 / 3.8 | 39.5 / 49.8 | 43.7 / 55.8 | 61 |
| `secp256k1` | off | 1.7 / 3.1 | 6.7 / 19.2 | 102.3 / 223.9 | 114.3 / 236.0 | 16 |
| `secp256k1` | on | 0.6 / 2.4 | 2.8 / 4.1 | 26.5 / 111.4 | 32.3 / 126.4 | 22 |

A 10-minute testnet run with `HL_SEND=1` on 2026-11-28 (BTC, default signer, `HL_SPREAD_BPS=1
HL_MIN_REQUOTE_MS=200`, no `HL_SPIN`) signed and sent 280 requotes and got 73 fills: build and sign
121.3 / 219.0 us, frame to signed 135.3 / 281.7 us, and 762.8 ms median from sending to the
venue's answer, over the websocket from Zurich. 40 modifies were refused because the quote had
filled first, which is normal, and nothing else was refused.

The benchmark (`bench/results/2026-11-28-001007-apple-m5-max.json`) measures the same steps in a
tight loop: 25.6 us to build, sign and write an order's body with the default signer and 12.6 us
with `secp256k1` (median), and 1,181,464 websocket frames a second, the fastest websocket parsing
of the SDKs benchmarked; with the `secp256k1` feature, df-hyperliquid-core has the fastest signing
of them.

Without `HL_SPIN` a requote takes four to five times as long as the benchmark. Nothing recovers a
signature on this path in any build; the difference comes from scheduling and caches. Between frames the
quoting thread sleeps, macOS runs it on a slower core or clock when it wakes, and the signing
code and tables have left the cache: the same signature takes about 23 us in a tight loop in this
program and about 52 us when it follows a pause. `HL_SPIN=1` keeps the thread busy and signs a
throwaway digest each idle millisecond, which brings a requote close to the benchmark's level at the
cost of one core at 100%. A requote also does more than the benchmark's single order: it hashes a
two-order `batchModify`.

Either way the SDK spends well under a millisecond per requote, against a network round trip of
hundreds of milliseconds (the same results file: 345.2 ms median for an info read and 375.4 ms for
an order, from Zurich). The SDK is not the bottleneck.

### What protects resting orders

- **The dead man's switch.** At the start it tries `scheduleCancel` once, 15 seconds ahead. The
  venue refuses the switch for accounts that have not traded enough (it asked one account for
  $1,000,000 of volume); the program then prints "scheduleCancel refused for this account; orders
  are cancelled on exit only." and never sends it again in that run, because each refresh is an
  action that spends the request budget (720 an hour at one every 5 seconds). Only an accepted
  switch is refreshed, every 5 seconds over the websocket, so the venue cancels every open order
  within 15 seconds of the program stopping. A refused refresh (the switch fires at most 10 times
  a day) stops the refreshing the same way.
- **When the switch fires**: the venue cancels every order and `orderUpdates` reports
  `scheduledCancel`. The program forgets those quotes, so it never modifies an order that is
  gone, and stops; with `HL_REQUOTE_AFTER_SWITCH=1` it prints that the switch fired and places new
  quotes.
- **Cancel on exit**: on Ctrl-C, when `HL_SECS` ends, at the loss limit and on any error.
- **The watchdog**: cancels everything when no book update, or no answer to a request, arrives
  for `HL_WATCHDOG_SECS` (default 10).

The gap that remains: if the process is killed, or the machine sleeps, while the network is also
down, and the venue refused the switch, the quotes rest until they fill or are cancelled by hand.

### When the venue says the address does not exist

The venue answers "User or API Wallet 0x... does not exist" when the address a signature recovers
to is not one it knows. The market maker then prints, in this order:

1. The likely causes: the account has never deposited; an agent (API) wallet is signing that has
   not been approved for this account, or was approved on the other network; the request was
   signed for one network and sent to the other; a vault or sub-account address is set when it
   should not be, or missing when it should be.
2. Every input of the hash: the nonce, the vault (or none), `expiresAfter` (or none), the network
   and its source letter (`a` for mainnet, `b` for testnet), and the URL the request went to.
3. Each stage of the signature from the crate's `Breakdown`: the msgpack bytes, the action hash,
   the EIP-712 payload, the digest, and the address the signature recovers to beside the key's own
   address.

It never prints the private key; a unit test checks that the printed text does not contain it.

### Requests and the rate limit

Orders never wait; reads do. Requotes go out over the websocket as soon as they are signed. The
dead man's switch and the cancels on exit go over HTTP through a client that sends no reads, so
no read's wait or 429 holds them. Every REST read (the startup snapshots, the request budget, the
open orders on exit) goes through `reads::read`, which yields to the quoting loop first, holds
the read while the program's last minute of HTTP weight is over half the venue's 1,200 a minute
per IP, and after a 429 holds every read until the cooldown ends. Under that, the crate's own gate
holds each client's reads at four fifths of the limit and closes for 2 seconds, doubling to a
minute, after a 429.

### The request budget

An address may send 10,000 requests plus one for every USDC it has traded, and each order or
cancel in a batch counts. At the start the program reads the address's budget and prints it; it
then keeps at least `HL_MIN_REQUOTE_MS` (default 500) between requotes, and waits longer as the
budget runs down (2 s under 5,000 left, 10 s under 1,000, 60 s under 200), so a new testnet
account does not run out mid-run.

## Checks

`just check` builds every example and runs the unit tests of the market maker's quoting,
inventory, timing, safety, read and printout logic (`cargo test -p df-hyperliquid-core --examples`).
