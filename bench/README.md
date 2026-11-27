# Benchmarks

## Reproduce

```sh
git clone https://github.com/dennisfurrer/df-hyperliquid-core
cd df-hyperliquid-core
just bench-compare
```

## Commands

Every command runs from the repository root. Each `just` command is a short name for the `cargo`
command beside it, so `just` is optional.

| `just` | `cargo` | What it runs |
|---|---|---|
| `just bench` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- run` | df-hyperliquid-core alone, offline |
| `just bench-compare` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- run --compare` | the same against hypersdk and the official Python SDK |
| `just bench-doctor` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- doctor` | which tools were found, and what a missing one skips |
| `just bench-share` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- share` | the latest results in at most 280 characters, to post |

Options for `bench` and `bench-compare`, added after the command (`just bench-compare --network`):

- `--network`: adds the info and exchange round trips on mainnet and the live websocket test.
  These depend mostly on the distance to the venue, not on the SDK.
- `--quick`: fewer samples and one run, to check that everything works. Quick results are marked
  as such, are not committed and `bench-share` refuses them.
- `--runs N`, `--rounds N`, `--live-secs N`: runs of each offline test (default 5), network
  rounds (default 100) and the live window in seconds (default 60).

No key is needed and none is ever read. The signing tests use a fixed public example key that
holds nothing. The exchange round trip (`--network` only) signs with a key each SDK generates
when it starts; the venue has no account for it and refuses the order, which is a post-only buy
30% under the market. Nothing can fill.

On an Apple M5 Max, `just bench` takes about 25 seconds and `just bench-compare` about 2 minutes,
with `--network` adding about 7 minutes. These times exclude the first build, which compiles
hypersdk, the crate and their dependencies and takes a few minutes; the runner says when it is
building, and every results file records the build and benchmark times separately.

## What you need

- Rust from [rustup](https://rustup.rs). The repository's `rust-toolchain.toml` picks the
  version (1.94.1); rustup installs it on the first command.
- Optional: a C compiler, for the `df-hyperliquid-core + secp256k1` row (libsecp256k1 is C).
  Without one, that row is skipped with a note.
- Optional: [uv](https://docs.astral.sh/uv/), for the Python SDK rows in `bench-compare`. uv
  installs the pinned Python and packages from `py/uv.lock`. Without it, those rows are skipped
  with a note.

`just bench-doctor` lists what it found. The offline tests make no network requests: the
recorded websocket frames are in `data/`, and only `--network` reaches the venue.

## What each test measures

- **Signature check.** Before any timing, every row signs one fixed order with the same key and
  nonce: a BTC limit buy (asset 0) at 50000 for 0.001, post-only, no client order id, grouping
  `na`, no vault, no expiry, mainnet, nonce 1700000000000, key
  `0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318` (the web3.js docs'
  example key). The run stops if any signature differs.
- **Hash + sign.** The order action already built in the SDK's own type, to the signature:
  msgpack encoding, the keccak connection id, the EIP-712 digest of the `Agent` struct and the
  secp256k1 RFC 6979 signature.
- **Build + sign + body.** From a typed price and size to the JSON body `/exchange` takes.
- **Recover.** df-hyperliquid-core only: the address a signature recovers to, which
  `Client::send` checks for a wallet signer before posting. A local key's signature is not
  recovered in a release build.
- **Websocket parse.** `data/frames.jsonl.gz` holds 4,000 text frames recorded from mainnet:
  2,000 `l2Book` frames from the 150 most traded perps and 2,000 `trades` frames from BTC, ETH,
  SOL, HYPE, XRP and DOGE. Each SDK decodes every frame the way its own websocket reader does,
  in timed passes, then once more with each frame timed on its own (the p99) and once more
  counting allocations.
- **Round trips** (`--network`). `POST /info` for BTC's `l2Book`, and the refused order above,
  from the call to the parsed answer, one request at a time with 0.3 s between requests and the
  order of the SDKs rotated each round.
- **Live websocket** (`--network`). Every SDK subscribes at once to BTC `l2Book` with
  `fast: true`, BTC `bbo` and BTC `trades`, and reports the frames it delivered and their age
  (local clock minus the venue's timestamp).

## Fairness notes

- Each Rust SDK is built into its own binary, one at a time, so cargo does not merge their
  dependency features: each binary is what a program using only that SDK would compile. Every
  call is the one the SDK's README or examples show, with nothing configured beyond the defaults.
- df-hyperliquid-core reads prices and sizes into its exact decimal `Dec`, and hypersdk into
  `rust_decimal::Decimal`. The Python SDK leaves them as strings in dicts, so its parse row is
  not a like-for-like comparison and is shown for reference.
- hypersdk's documented signing call, `Action::sign_sync`, takes the action by value, so its
  timed call signs a clone. The clone is timed on its own and reported under the signing table;
  it takes tens of nanoseconds against about 34 us for the signature.
- hypersdk is timed decoding with `serde_json::from_slice::<Incoming>`, which is what its
  websocket stream calls on each text frame.
- The `df-hyperliquid-core + secp256k1` row is an optional feature (libsecp256k1 instead of
  k256), not the default build. The default build's row is the first one.
- The Python SDK signs with pure Python ECDSA unless coincurve is installed; both rows are shown,
  the backend chosen with `ECC_BACKEND_CLASS`.
- The parse corpus has `l2Book` and `trades` frames only. df-hyperliquid-core's `bbo`, fast-book
  and bucketed-book readers are checked against 1,500 more recorded frames in
  `data/coverage.jsonl.gz`, which is not timed.

The harness for each competitor is in `hypersdk/` and `py/`. If an SDK has a faster documented
way to do any of these steps, a pull request that uses it is welcome, and the next published
results will include it.

## Platform effects

- Keccak runs on the ARMv8.2 SHA3 instructions on Apple silicon and other ARMv8.2 CPUs, and on
  the portable path elsewhere, including x86-64. The signing gap may differ on x86-64. The
  published results are from Apple silicon; results from other machines are welcome (below).
- Laptops on battery, low-power modes and other load change the figures. The offline tests run
  one process at a time.
- The Python figures depend on the CPython version, which `py/uv.lock` pins (3.13).

## Method

- Signing: 2,000 warm-up calls, then 20,000 timed calls, each timed on its own. Python: 200
  warm-up calls, then 2,000 timed calls (pure Python) or 5,000 (coincurve).
- Parse: 3 untimed passes over the corpus, then 20 timed passes; frames per second are over the
  timed passes.
- Each offline test runs 5 times as a separate process. The run with the median result is
  reported, and every run's figure is kept in the JSON under `runs`.
- `--quick`: a tenth of the signing samples, a quarter of the parse passes and one run.

## Results files

Every run writes `results/<date>-<hhmmss>-<cpu>.json`, `.md` and `.pdf`; `-df` marks a
df-hyperliquid-core run alone. Each records the CPU, cores, memory, OS and architecture (never the
host or user name), the rustc version, df-hyperliquid-core's version and git commit, the
competitors' pinned versions, the options, every run's figures, every raw network sample, what
was skipped, and the build and benchmark times. The `.pdf` is the same tables laid out to read
and share.

Competitor versions are pinned: hypersdk through `Cargo.lock` (built with `--locked`), the Python
SDK and its dependencies through `py/uv.lock`.

## Results

From `results/2026-11-27-180238-apple-m5-max.json` (also `.md` and `.pdf`), one
`just bench-compare --network` run on an Apple M5 Max in Zurich. Against hypersdk 0.3.1,
df-hyperliquid-core parsed websocket frames 2.4 times as fast (2.7 times on `l2Book`) with fewer
allocations and a lower p99 per frame, and signed orders 1.3 times as fast with its defaults and
2.7 times as fast with the `secp256k1` feature. The round trips and the live websocket were level
for all three SDKs. These are Apple silicon figures; see Platform effects.
Machine: Apple M5 Max, 18 cores, 128 GB, macOS 26.5.2, aarch64  
Versions: df-hyperliquid-core 0.2.3 (commit ba02703), hypersdk 0.3.1, hyperliquid-python-sdk 0.24.0; rustc 1.94.1 (e408947bf 2026-03-25)  
Run: compare, offline and network, 5 runs per row with the median run reported, started 2026-11-27T18:02:38Z  

### Signing one BTC limit order (offline)

| SDK | hash + sign median (us) | p99 (us) | build + sign + body median (us) | p99 (us) |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 27.0 | 31.5 | 28.3 | 33.2 |
| df-hyperliquid-core + secp256k1 | **13.2** | 20.5 | 14.8 | 15.5 |
| hypersdk | 35.4 | 46.0 | 35.9 | 40.2 |
| hyperliquid-python-sdk | 2058.5 | 2386.5 | 2065.3 | 2166.9 |
| hyperliquid-python-sdk + coincurve | 127.2 | 155.2 | 133.0 | 159.5 |

Same signature from every row: yes. hypersdk's sign_sync takes the action by value, so its timed call signs a clone; the clone alone takes 41 ns at the median.

### Recovering the signer from a signature (offline)

| SDK | median (us) | p99 (us) |
|---|---:|---:|
| df-hyperliquid-core | 81.7 | 134.5 |
| df-hyperliquid-core + secp256k1 | **14.9** | 15.7 |

Client::send recovers a wallet's signature before posting; a local key's is not recovered in a release build.

### Websocket parse (4,000 recorded mainnet frames, offline)

| SDK | all frames/s | l2Book frames/s | trades frames/s | p99 per frame (ns) | allocations per frame | not read |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | **1,113,937** | 787,458 | 1,971,042 | 2,833 | 4.7 | 0 |
| hypersdk | 458,203 | 288,201 | 1,267,566 | 4,667 | 7.7 | 0 |
| hyperliquid-python-sdk | 216,734 | 144,534 | 445,214 | 10,666 | - | 0 |

The Python SDK leaves prices and sizes as strings; the two Rust SDKs read them into exact decimals.

### Info round trip: POST /info l2Book BTC (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | failed |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 232.4 | 273.1 | 313.0 | 0 |
| hypersdk | 233.2 | 273.5 | 429.6 | 0 |
| hyperliquid-python-sdk | 234.0 | 274.7 | 362.0 | 0 |

Almost all of a round trip is the network and the venue, so these show parity, not a ranking.

### Exchange round trip: refused post-only order from an empty key (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | failed |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 242.3 | 295.7 | 332.8 | 0 |
| hypersdk | 246.2 | 272.1 | 385.9 | 0 |
| hyperliquid-python-sdk | 250.3 | 281.0 | 332.6 | 0 |

Almost all of a round trip is the network and the venue, so these show parity, not a ranking.

### Live websocket, BTC l2Book (fast), bbo and trades, 60 s, every SDK at once

| SDK | l2Book | bbo | trades frames | not read | age median (ms) | age p99 (ms) |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 110 | 579 | 120 | 0 | 360 | 688 |
| hypersdk | 110 | 578 | 120 | 0 | 344 | 633 |
| hyperliquid-python-sdk | 110 | 576 | 119 | - | 335 | 665 |

Age is the local clock minus the venue's timestamp; it compares the SDKs with each other.

## Submit your results

Run `just bench-compare`, then open an issue with the
[benchmark result template](https://github.com/dennisfurrer/df-hyperliquid-core/issues/new?template=benchmark-result.yml)
and paste the JSON file from `bench/results/`. `just bench-share` prints a line to post.

## Recording a new corpus

```sh
cd bench/py && uv run python record.py --out ../data/frames.jsonl.gz --per-channel 2000 --books 150
cd bench/py && uv run python record.py --coverage --out ../data/coverage.jsonl.gz --per-channel 500 --books 20
```

## Files

- `runner/`: the runner the commands above start: builds, runs, writes the results files.
- `common/`: what the two Rust benchmark binaries share: the fixed signing input, the timing
  harness and the corpus loader.
- `df/`, `hypersdk/`: one benchmark binary per Rust SDK.
- `py/`: the Python SDK benchmark (`bench_py.py`), the corpus recorder (`record.py`) and the
  pinned uv project.
- `data/`: the recorded websocket corpora.
- `results/`: one set of files per run. `2026-11-24.*` and `2026-11-27.*` are from the earlier
  `run.py` harness.
