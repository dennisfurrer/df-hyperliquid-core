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
| df-hyperliquid-core | 0.0.12-beta, commit 846cb0d, by path |
| hypersdk | 0.3.1 |
| hyperliquid-python-sdk | 0.24.0, with eth-account 0.13.7, eth-keys 0.8.0, requests 2.34.2, websocket-client 1.9.2 |
| coincurve | 21.0.0, for one extra signing row only (see below) |
| Network | Zurich, Switzerland. `api.hyperliquid.xyz` was served by CloudFront edge ZRH52 |
| Date | 2026-11-24 |

Each Rust SDK is built into its own binary (`bench-df`, `bench-hypersdk`), and each binary is
built on its own, so cargo does not merge the two SDKs' dependency features. Each binary is
what a program using only that SDK would compile. All calls are the ones each SDK's README or
examples show; nothing is configured beyond the defaults.

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

Iterations: Rust 2,000 warm-up calls, then 20,000 timed calls. Python 200 warm-up calls, then
2,000 timed calls (pure Python ECDSA) or 5,000 (coincurve). ops/s is 1 / mean time.

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

- df-hyperliquid-core: `PlaceOrder::one(Order::limit(...))` and `client.send(&place)`. `send`
  also recovers the signature and checks it against the signer before posting.
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
passes. The `l2Book` and `trades` columns repeat this over each channel's frames alone.

**Live.** The three SDKs run at the same time for the same 60 s, each subscribed to BTC
`l2Book` and BTC `trades`. Reported: frames delivered to the program as typed values (dicts for
Python), frames the SDK could not read, and the age of each frame when the program got it:
local clock minus the venue's timestamp (the book's `time`, the newest trade's `time`). The age
includes the network path from the venue, the venue's own publish delay and any clock offset,
so it compares the three SDKs with each other and is not an absolute latency. The time from
receiving a frame to the typed value cannot be measured through any of the three public APIs,
since all three decode inside their own reader task; the offline ns per frame above stands in
for it. During this run the venue sent a BTC `l2Book` frame only every 5 to 12 seconds. The
small differences in frame counts come from each SDK connecting and subscribing at a slightly
different moment (the Python `Info` client also reads the spot and perp metadata first).

## Results

From `results/2026-11-24.md`.

### Signing one BTC limit order (offline)

| SDK | hash + sign median (us) | p99 (us) | ops/s | build + sign + body median (us) | p99 (us) | ops/s |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 25.5 | 27.6 | 38,279 | 27.1 | 29.0 | 36,229 |
| hypersdk | 35.7 | 36.4 | 28,049 | 36.2 | 40.5 | 27,625 |
| hyperliquid-python-sdk | 2056.7 | 2161.2 | 483 | 2067.3 | 2185.7 | 482 |
| hyperliquid-python-sdk + coincurve | 129.1 | 161.8 | 7,650 | 139.3 | 230.1 | 6,778 |

### Info round trip: POST /info l2Book BTC (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | min (ms) | max (ms) | failed |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 258.57 | 318.44 | 459.98 | 243.85 | 665.11 | 0 |
| hypersdk | 257.12 | 321.05 | 387.24 | 244.90 | 411.82 | 0 |
| hyperliquid-python-sdk | 263.65 | 308.03 | 592.87 | 250.54 | 593.31 | 0 |

### Exchange round trip: sign and POST /exchange, refused order (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | min (ms) | max (ms) | failed |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 277.50 | 356.97 | 539.74 | 258.20 | 545.94 | 0 |
| hypersdk | 275.36 | 331.64 | 434.72 | 254.90 | 542.55 | 0 |
| hyperliquid-python-sdk | 279.39 | 346.12 | 568.69 | 258.83 | 680.14 | 0 |

### Websocket parse throughput (4,000 recorded frames)

| SDK | all frames/s | all MB/s | l2Book frames/s | trades frames/s | ns per frame | frames not read |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 252,879 | 294.3 | 161,122 | 676,054 | 3,954 | 0 |
| hypersdk | 447,251 | 520.5 | 290,463 | 1,263,766 | 2,236 | 0 |
| hyperliquid-python-sdk (untyped dicts) | 218,252 | 254.0 | 144,198 | 464,472 | 4,582 | 0 |

### Live websocket, BTC l2Book and trades, 60 s, all three at once

| SDK | l2Book frames | trades frames | trades | frames not read | age median (ms) | age p99 (ms) |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 6 | 167 | 683 | 0 | 418 | 626 |
| hypersdk | 6 | 168 | 685 | 0 | 404 | 665 |
| hyperliquid-python-sdk | 6 | 165 | 679 | not reported | 382 | 557 |

### What the numbers show

- **Signing:** df-hyperliquid-core is the fastest: 25.5 us median, 1.4 times faster than
  hypersdk (35.7 us), 5 times faster than the Python SDK with coincurve (129 us) and 80 times
  faster than the Python SDK as installed by default (2.06 ms).
- **Websocket parsing:** hypersdk is the fastest: 1.77 times the frames/s of
  df-hyperliquid-core over the whole corpus (447k against 253k), 1.80 times on `l2Book` and
  1.87 times on `trades`. df-hyperliquid-core decodes prices and sizes into its own exact decimal
  type; it is about 16% faster than the Python SDK's untyped `json.loads`.
- **Info and exchange round trips:** the three SDKs are within 7 ms of each other at the
  median (257 to 264 ms for info, 275 to 279 ms for exchange). Almost all of each round trip
  is network and venue time; the differences between the SDKs are smaller than the variation
  between requests, and hypersdk had the lowest median and p99 on both in this run. Even the
  Python SDK's 2 ms of pure Python signing is under 1% of an exchange round trip from Zurich.
- **Live websocket:** all three received about the same number of frames (6 books, 165
  to 168 trade frames) and the two Rust SDKs reported no frames they could not read. The age
  differences (382 to 418 ms median) are within the spread of the samples and are not a
  ranking.

## Rerun

From the repository root:

```sh
BENCH_LOCATION="City, Country" bench/run.sh
```

It builds both Rust binaries, syncs the Python environment with uv, runs everything and writes
`bench/results/<date>.json` (every raw network sample, the signature check and the machine) and
`bench/results/<date>.md`. It needs rustup, uv and network access to `api.hyperliquid.xyz`; it
takes about 6 minutes, most of it the paced network rounds. `BENCH_ROUNDS`, `BENCH_GAP_S` and
`BENCH_LIVE_SECS` change the round count (default 100), the gap between requests (0.3 s) and
the live window (60 s).

To record a new websocket corpus (about 6 minutes):

```sh
cd bench/py && uv run python record.py --out ../data/frames.jsonl.gz --per-channel 2000 --books 150
```

Single parts can be run by hand, for example:

```sh
cd bench
cargo build --release -p bench-df && cargo build --release -p bench-hypersdk
./target/release/bench-df sign
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
- `results/`: one JSON and one markdown file per run.
