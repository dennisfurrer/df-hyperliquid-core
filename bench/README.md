# Benchmarks

df-hyperliquid-core compared with [hypersdk](https://crates.io/crates/hypersdk) (Rust) and the
official [hyperliquid-python-sdk](https://pypi.org/project/hyperliquid-python-sdk/), on one
machine and one network, run back to back.

This directory is its own Cargo workspace. hypersdk is a dependency only here, so it is not in
the crate's own `Cargo.lock`.

## Method

### Machine and versions

| | |
|---|---|
| Machine | Mac17,6, Apple M5 Max, 18 cores, 128 GB |
| OS | macOS 26.5.2 |
| Rust | 1.94.1 (the repo's `rust-toolchain.toml`), release profile: opt-level 3, no LTO |
| Python | CPython 3.13.12, environment from `py/uv.lock` |
| df-hyperliquid-core | 0.2.2 and the changes after it, commit f641b63, by path |
| hypersdk | 0.3.1 |
| hyperliquid-python-sdk | 0.24.0, with eth-account 0.13.7, eth-keys 0.8.0, requests 2.34.2, websocket-client 1.9.2 |
| coincurve | 21.0.0, for one extra signing row only (see below) |
| Network | Zurich, Switzerland. `api.hyperliquid.xyz` was served by CloudFront edge ZRH52 |
| Date | 2026-11-27 |

Each Rust SDK is built into its own binary (`bench-df`, `bench-hypersdk`), and each binary is
built on its own, so cargo does not merge the two SDKs' dependency features. Each binary is
what a program using only that SDK would compile. All calls are the ones each SDK's README or
examples show; nothing is configured beyond the defaults. df-hyperliquid-core is also built a
second time with its `secp256k1` feature (signing and recovery through libsecp256k1), into its
own target directory, and shown as its own row, "df-hyperliquid-core + secp256k1", in the
signing tables.

### Fixed signing input

- Key: the public example key from the web3.js docs, `0x4c08...2318`. It holds nothing.
- Action: one BTC limit buy (asset 0), price 50000, size 0.001, TIF `Alo` (post-only), no
  client order id, grouping `na`, no vault, no expiry, mainnet.
- Nonce: 1700000000000.

Before any timing, `run.py` has every SDK sign this action and compares the results. All
three SDKs (and the Python SDK with either ECDSA backend) produced the same signature,
`0x02af7a31...5cd8001c`, for the address `0x2c7536e3605d9c16a7a3d7b1898e529396a65c23`. The run
stops if they differ.

### 1. Signing time (offline)

Two measurements, each one call timed on its own (`std::time::Instant` in Rust,
`time.perf_counter_ns` in Python):

- **hash + sign**: the order action already built in the SDK's own type. Includes msgpack
  encoding, the keccak connection id, the EIP-712 digest of the `Agent` struct and the
  secp256k1 RFC 6979 signature. Excludes key parsing, building the action and serializing the
  body.
  - df-hyperliquid-core: `key.sign_digest(&place.digest(nonce, None, Network::Mainnet))`.
  - hypersdk: `Action::Order(batch).sign_sync(&signer, nonce, None, None, Chain::Mainnet)`.
    `sign_sync` takes the action by value, so each call signs a clone of it.
  - Python: `sign_l1_action(wallet, action, None, nonce, None, True)`.
- **build + sign + body**: from a typed price and size (`Dec`, `Decimal`, `float`) to the JSON
  body that `/exchange` takes: build the order, hash, sign and serialize.

- **recover**: df-hyperliquid-core only. `account::recover(digest, signature)`, the step
  `Client::send` runs for a wallet signer before posting. For a `LocalKey`, `send` skips it in a
  release build, since the key signed with the key its address comes from. hypersdk has no such
  step.

Iterations: Rust 2,000 warm-up calls, then 20,000 timed calls. Python 200 warm-up calls, then
2,000 timed calls (pure Python ECDSA) or 5,000 (coincurve). ops/s is 1 / mean time. Each Rust
binary is run 5 times (`BENCH_OFFLINE_RUNS`); the run with the median hash + sign time is
reported, and every run's median is kept in the JSON.

The Python SDK's dependencies sign with eth-keys' pure Python ECDSA unless coincurve is
installed. That default is the main row. A second row, "+ coincurve", shows the same SDK with
the libsecp256k1 backend that eth-keys uses when coincurve is present. The backend is chosen
with `ECC_BACKEND_CLASS` for every Python run.

### 2. Info round trip

`POST /info {"type":"l2Book","coin":"BTC"}` on mainnet, timed from the call to the parsed
value:

- df-hyperliquid-core: `markets::book::l2_book(&client, "BTC", Precision::FULL)`, a `Book` of
  exact decimals.
- hypersdk: `client.l2_book("BTC".into(), None, None)`, an `L2Book` of `Decimal`.
- Python: `Info.l2_snapshot("BTC")`, a dict of strings.

Includes building the request, the HTTP POST and parsing the answer. Excludes creating the
client and the first connection: each SDK keeps one long-lived process and connection, and the
first 3 rounds are discarded. The venue answers over HTTP/1.1 (it did not take HTTP/2 when
offered), so all three use a kept-alive HTTP/1.1 connection.

### 3. Exchange round trip

Each SDK generates a fresh random key at start. The key has never held anything, so the venue
has no account for it and refuses the order: every answer was
`User or API Wallet 0x... does not exist.` Nothing can trade. The order is a post-only BTC buy
of 0.00029 at 42023 (30% under the mid at the time, about $12).

Timed from building the order to the parsed answer:

- df-hyperliquid-core: `PlaceOrder::one(Order::limit(...))` and `client.send(&place)`. With a
  `LocalKey`, `send` does not recover the signature in a release build; with a wallet signer it
  would.
  The refusal comes back as `ApiError::Refused`.
- hypersdk: `client.place(&signer, batch, nonce, None, None)`. The refusal comes back as an
  `ActionError`.
- Python: `Exchange.order("BTC", True, sz, px, {"limit": {"tif": "Alo"}})`, with the default
  pure Python ECDSA. The refusal comes back as a dict.

### Pacing of the network tests

100 measured rounds after 3 warm-up rounds. In each round every SDK makes one info request,
then every SDK makes one exchange request, one request at a time with 0.3 s between requests.
The order of the SDKs rotates every round. One round weighs 9 (three `l2Book` reads at 2,
three actions at 1), about 250 weight a minute, well under the 1200 a minute IP limit. All
raw samples are in `results/2026-11-24.json`.

### 4. Websocket

**Parse throughput.** `data/frames.jsonl.gz` holds 4,000 text frames recorded from mainnet by
`py/record.py`: 2,000 `l2Book` frames from the 150 most traded perps and 2,000 `trades` frames
from BTC, ETH, SOL, HYPE, XRP and DOGE, 4.65 MB in all. Each SDK decodes every frame the way
its own websocket reader does:

- df-hyperliquid-core: `live::decode(text)` into a typed `Frame`.
- hypersdk: `serde_json::from_slice::<Incoming>(bytes)`, which is what its stream calls.
- Python: `json.loads(text)` and `ws_msg_to_identifier`, which is what `WebsocketManager`
  does before calling the subscriber. The result is a dict of strings: prices and sizes are
  not converted to numbers. This row is not a like-for-like comparison with the two typed
  decoders and is shown for reference.

3 untimed passes, then 20 timed passes over all frames; frames/s and MB/s are over the timed
passes. The `l2Book` and `trades` columns repeat this over each channel's frames alone. Then
one pass with each frame timed on its own gives the p99 time of one frame, and one more pass
counts allocations (the Rust binaries install a counting global allocator; one relaxed atomic
add per allocation, the same for both). Each SDK's parse is run 5 times and the run with the
median frames/s is reported.

`data/coverage.jsonl.gz` holds 1,500 more mainnet frames (500 `bbo`, 500 fast books, 500 books
at five significant figures with `spread`), recorded with `record.py --coverage`. It is not
timed: df-hyperliquid-core's tests read it to check its direct frame reader against serde.

**Live.** The three SDKs run at the same time for the same 60 s, each subscribed to BTC `l2Book`
with `fast: true`, BTC `bbo` and BTC `trades`. A book without `fast` came about every 7 s on
2026-11-27 (4 frames in 30 s, against 57 with `fast` and 324 `bbo` frames), too few to compare
frame ages, so all three subscribe the same fast book. A fast book has 5 levels a side. Reported: frames delivered to the program as typed values (dicts for
Python), frames the SDK could not read, and the age of each frame when the program got it:
local clock minus the venue's timestamp (the book's `time`, the bbo's `time`, the newest trade's `time`). The age
includes the network path from the venue, the venue's own publish delay and any clock offset,
so it compares the three SDKs with each other and is not an absolute latency. The time from
receiving a frame to the typed value cannot be measured through any of the three public APIs,
since all three decode inside their own reader task; the offline ns per frame above stands in
for it. The
small differences in frame counts come from each SDK connecting and subscribing at a slightly
different moment (the Python `Info` client also reads the spot and perp metadata first).

## Results

From `results/2026-11-27.md`.

### Signing one BTC limit order (offline)

Same signature from all SDKs: yes (`0x02af7a312db30840...`)

| SDK | hash + sign median (us) | p99 (us) | ops/s | build + sign + body median (us) | p99 (us) | ops/s |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 25.6 | 28.3 | 38,147 | 26.7 | 28.8 | 36,555 |
| df-hyperliquid-core + secp256k1 | 13.1 | 13.5 | 77,727 | 14.5 | 14.9 | 70,021 |
| hypersdk | 33.8 | 36.2 | 29,217 | 34.3 | 37.0 | 28,735 |
| hyperliquid-python-sdk | 2017.0 | 2055.9 | 499 | 2023.6 | 2064.5 | 498 |
| hyperliquid-python-sdk + coincurve | 122.2 | 145.9 | 8,222 | 127.5 | 150.2 | 7,886 |

### Recovering the signer from a signature (offline)

| SDK | median (us) | p99 (us) | ops/s |
|---|---:|---:|---:|
| df-hyperliquid-core | 79.6 | 85.2 | 12,610 |
| df-hyperliquid-core + secp256k1 | 14.8 | 15.1 | 68,715 |

### Info round trip: POST /info l2Book BTC (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | min (ms) | max (ms) | failed |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 237.93 | 303.28 | 409.12 | 228.21 | 470.04 | 0 |
| hypersdk | 238.78 | 307.91 | 418.50 | 227.12 | 1228.27 | 0 |
| hyperliquid-python-sdk | 238.70 | 297.76 | 411.61 | 229.36 | 415.37 | 0 |

### Exchange round trip: sign and POST /exchange, refused order (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | min (ms) | max (ms) | failed |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 266.89 | 361.75 | 461.36 | 236.30 | 479.73 | 0 |
| hypersdk | 262.71 | 351.25 | 453.24 | 236.96 | 558.96 | 0 |
| hyperliquid-python-sdk | 257.11 | 339.92 | 461.65 | 242.65 | 508.78 | 0 |

### Websocket parse throughput (4000 recorded frames)

| SDK | all frames/s | all MB/s | l2Book frames/s | trades frames/s | ns per frame | p99 ns per frame | allocations per frame | frames not read |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 1,145,475 | 1,332.9 | 823,194 | 2,000,892 | 873 | 2,875 | 4.7 | 0 |
| hypersdk | 477,986 | 556.2 | 301,107 | 1,271,890 | 2,092 | 4,667 | 7.7 | 0 |
| hyperliquid-python-sdk | 224,450 | 261.2 | 147,679 | 474,833 | 4,455 | 10,542 | - | 0 |

### Live websocket, BTC l2Book (fast), bbo and trades, 60 s, all three at once

| SDK | l2Book frames | bbo frames | trades frames | trades | frames not read | age median (ms) | age p99 (ms) |
|---|---:|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 111 | 667 | 132 | 472 | 0 | 354 | 636 |
| hypersdk | 111 | 667 | 132 | 472 | 0 | 363 | 643 |
| hyperliquid-python-sdk | 110 | 656 | 132 | 472 | not reported | 351 | 621 |

### What the numbers show

- **Signing:** df-hyperliquid-core is the fastest. With its defaults (k256, pure Rust) it signs in
  25.6 us at the median, 1.3 times faster than hypersdk (33.8 us). With the `secp256k1` feature
  it signs in 13.1 us, 2.6 times faster than hypersdk, 9 times faster than the Python SDK with
  coincurve and 150 times faster than the Python SDK as installed by default. Over the 5 runs the
  medians varied by under 6% for every Rust row.
- **Recovery:** 79.6 us with k256 and 14.8 us with libsecp256k1. `Client::send` recovers only a
  wallet's signature; a `LocalKey`'s is not recovered in a release build.
- **Websocket parsing:** df-hyperliquid-core is the fastest: 2.4 times the frames/s of hypersdk
  over the whole corpus (1.15 million against 478 thousand), 2.7 times on `l2Book` and 1.6 times
  on `trades`, with a lower p99 per frame (2.9 us against 4.7 us) and fewer allocations per
  frame (4.7 against 7.7; on `l2Book` 3 against 9, on `trades` the same). It reads prices and
  sizes into exact decimals, as hypersdk does into `Decimal`. In the 2026-11-24 run, before
  these changes, hypersdk parsed 1.8 times as fast as df-hyperliquid-core.
- **Info and exchange round trips:** the three SDKs are at parity. The medians are within 1 ms
  of each other for info (238 ms) and within 10 ms for exchange (257 to 267 ms), against a p90
  to median spread of about 70 to 100 ms. Almost all of each round trip is network and venue
  time; the SDKs' own work is under 0.1 ms of it. These tables show that no SDK adds latency,
  not a ranking.
- **Live websocket:** with the same fast book, bbo and trades subscriptions, all three received
  the same frames (111 books, 667 bbo, 132 trade frames for the two Rust SDKs) and the two Rust
  SDKs reported no frames they could not read. The frame ages (351 to 363 ms median) are within
  the spread of the samples and are not a ranking.

## Rerun

From the repository root:

```sh
BENCH_LOCATION="City, Country" bench/run.sh
```

It builds the three Rust binaries, syncs the Python environment with uv, runs everything and
writes `bench/results/<date>.json` (every raw network sample, every offline run's median, the
signature check and the machine) and `bench/results/<date>.md`. It needs rustup, a C compiler
(for libsecp256k1), uv and network access to `api.hyperliquid.xyz`; it takes about 12 minutes,
most of it the paced network rounds. `BENCH_ROUNDS`, `BENCH_GAP_S`, `BENCH_LIVE_SECS` and
`BENCH_OFFLINE_RUNS` change the round count (default 100), the gap between requests (0.3 s), the
live window (60 s) and the runs of each offline benchmark (5).

To record a new websocket corpus (about 6 minutes):

```sh
cd bench/py && uv run python record.py --out ../data/frames.jsonl.gz --per-channel 2000 --books 150
```

Single parts can be run by hand, for example:

```sh
cd bench
cargo build --release -p bench-df && cargo build --release -p bench-hypersdk
cargo build --release -p bench-df --features secp256k1 --target-dir target/secp256k1
./target/release/bench-df sign
./target/secp256k1/release/bench-df sign
./target/release/bench-hypersdk parse --corpus data/frames.jsonl.gz
ECC_BACKEND_CLASS=eth_keys.backends.NativeECCBackend uv run --project py python py/bench_py.py sign
```

## Files

- `common/`: the fixed signing input, timing harness, statistics and corpus loader shared by
  the Rust binaries.
- `df/`, `hypersdk/`: one benchmark binary per Rust SDK.
- `py/`: the Python SDK benchmark (`bench_py.py`), the corpus recorder (`record.py`) and the
  pinned uv project.
- `run.py`, `run.sh`: the entry point.
- `data/frames.jsonl.gz`: the recorded websocket corpus.
- `data/coverage.jsonl.gz`: bbo, fast and bucketed book frames for df-hyperliquid-core's tests;
  not timed.
- `results/`: one JSON and one markdown file per run.
