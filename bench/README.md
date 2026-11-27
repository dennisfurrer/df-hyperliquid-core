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

On an Apple M5 Max, `just bench` takes about 25 seconds and `just bench-compare` about 3 minutes,
with `--network` adding about 12 minutes. These times exclude the first build, which compiles
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

To be filled from the committed run.

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
