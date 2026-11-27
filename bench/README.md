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
| `just bench-compare` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- run --compare` | the same against hypersdk, hyperliquid-sdk-rs, quicknode-hyperliquid-sdk, hyperliquid_rust_sdk, @nktkas/hyperliquid, go-hyperliquid and the official Python SDK |
| `just bench-doctor` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- doctor` | which tools were found, and what a missing one skips |
| `just bench-share` | `cargo run --release --locked --manifest-path bench/Cargo.toml -p bench-runner -- share` | the latest results in at most 280 characters, to post: df-hyperliquid-core's speed divided by each competitor's, for parsing and signing, where a figure below 1 means that competitor was faster; a run without competitors says "df-hyperliquid-core only, no comparison" |

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

On an Apple M5 Max, `just bench` takes about 25 seconds and `just bench-compare` about 3.5
minutes, with `--network` adding about 23 minutes (100 rounds of two requests for eight SDKs,
0.3 s apart, then the 60 s live window). These times exclude the first build, which compiles
every SDK and its dependencies and takes several minutes; the runner says when it is building,
and every results file records the build and benchmark times separately.

## What you need

- Rust from [rustup](https://rustup.rs). The repository's `rust-toolchain.toml` picks the
  version (1.94.1); rustup installs it on the first command.
- Optional: a C compiler, for the `df-hyperliquid-core + secp256k1` row (libsecp256k1 is C).
  Without one, that row is skipped with a note.
- Optional: [uv](https://docs.astral.sh/uv/), for the Python SDK rows in `bench-compare`. uv
  installs the pinned Python and packages from `py/uv.lock`. Without it, those rows are skipped
  with a note.
- Optional: [Node](https://nodejs.org) 22.12 or newer and [pnpm](https://pnpm.io), for the
  @nktkas/hyperliquid rows. The runner runs `pnpm install --frozen-lockfile` in `ts/`. Without
  either, those rows are skipped with a note.
- Optional: [Go](https://go.dev/dl/) 1.25.3 or newer, for the go-hyperliquid rows, built from
  `go/go.mod` and `go/go.sum`. Without it, those rows are skipped with a note.
- Optional: `protoc` (`brew install protobuf`, or the protobuf package of your Linux
  distribution), for the quicknode-hyperliquid-sdk rows: that crate's build script compiles
  protobuf files. Without it, those rows are skipped with a note.

`just bench-doctor` lists what it found. The offline tests make no network requests: the
recorded websocket frames are in `data/`, and only `--network` reaches the venue.

## What each test measures

- **Signature check.** Before any timing, every row signs one fixed order with the same key and
  nonce: a BTC limit buy (asset 0) at 50000 for 0.001, post-only, no client order id, grouping
  `na`, no vault, no expiry, mainnet, nonce 1700000000000, key
  `0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318` (the web3.js docs'
  example key). A row whose signature, signer address or action differs from
  df-hyperliquid-core's is still timed, and the signing table names it as failing the check
  with the reason.
- **Hash + sign.** The order action already built in the SDK's own type, to the signature:
  msgpack encoding, the keccak connection id, the EIP-712 digest of the `Agent` struct and the
  secp256k1 RFC 6979 signature.
- **Build + sign + body.** From the price and size a user hands the SDK (typed decimals, strings
  or floats, whichever the SDK takes) to the JSON body `/exchange` takes.
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
  (local clock minus the venue's timestamp). An SDK without one of these subscriptions or
  options gets the closest one it has; the table's note names them.

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

hyperliquid-sdk-rs 0.1.2 (lhermoso, `lhermoso/`):

- It has no public call that signs without sending and takes the nonce from the clock, so the
  harness runs the steps of `RawExchangeProvider::place_order` with the SDK's public types
  (`OrderRequest::limit`, `BulkOrder`, the `Agent` EIP-712 struct, its `HyperliquidSigner` for
  alloy's `PrivateKeySigner`) and a copy of its private `hash_action`, restricted to the `order`
  action. Its signer is async; the future is ready at once and is polled on the calling thread.
- The signing step is therefore a copy of its private code. The harness's `verify-copy` checks
  the copy byte for byte against the SDK itself: it runs the SDK's public `place_order` for the
  fixed order with a signer that records the EIP-712 hash it is asked to sign and then refuses,
  so nothing is sent, and the copy must give the same hash at the nonce the SDK took from the
  clock and the same signature bytes. Every compare run records the result under `copy_check`,
  and the signing table's note states it. A public call from the maintainer that signs an order
  without sending it, with a given nonce, is welcome; the harness would then time that call
  instead of the copy.
- Parse times what its websocket provider does with each text frame: copy the bytes (it copies
  them out of the frame too) and decode them in place with `simd_json::from_slice::<Message>`.
  It then clones each message once per subscription, which is not timed.
- Prices and sizes stay strings.
- Its l2Book subscription has no `fast` option, and its message router sends every message to
  every subscription's channel, so the live run reads one channel. It drops a frame it cannot
  read without a trace, so "not read" is not counted.
- Its examples install rustls's aws-lc-rs provider at start; the harness does the same, because
  without it rustls panics on the first TLS connection (its dependency graph enables both of
  rustls's providers).

quicknode-hyperliquid-sdk 0.1.9 (QuickNode, `quicknode/`):

- Its order methods do not hash locally: they send the action to QuickNode's worker
  (`send.hyperliquidapi.com`), sign the hash the worker returns and send the order through the
  worker. The orders sent this way carry a builder fee set by the worker; the refusals recorded
  here show 0.04%. The offline rows time its public `signing::sign_action`, which hashes and
  signs locally, on the action JSON its `place_order` builds for a limit order, without the
  random client order id it adds.
- Its round trips go through QuickNode's own service, not to the venue's API directly: the
  exchange round trip is two requests to the worker (build, then send), and the info round trip
  goes to the worker's info relay, or to the QuickNode endpoint when one is set in
  `QUICKNODE_ENDPOINT`. Its row in the round-trip tables is labelled "via QuickNode", the
  table's note says its requests go through QuickNode's service, and it is not ranked with the
  others.
- Parse times `serde_json::from_str::<Value>` with the `preserve_order` feature it turns on, which
  is what its `Stream` does with each frame. Its callbacks then each get a clone, not timed.
- Its `Stream` documents a QuickNode endpoint. Without one, it sends its subscriptions to the
  public websocket in a shape the venue refuses. The live row is therefore shown as not
  applicable and left out of the age columns unless `QUICKNODE_ENDPOINT` is set, in which case
  the harness passes that endpoint to `Stream::new` and the row is measured like the others.
- Its build script compiles protobuf files, so the build needs `protoc`.

hyperliquid_rust_sdk 0.6.0 (Hyperliquid's official Rust SDK, `official-rust/`):

- Its signing functions, its wire order type and its order conversion are private, and it takes
  the nonce from the clock. The harness runs the steps of `ExchangeClient::bulk_order` with its
  public `Actions`, `BulkOrder` and `ClientOrderRequest` types and copies of its private
  `Actions::hash`, `float_to_string_for_hashing`, `ClientOrderRequest::convert`,
  `sign_l1_action`, the `Agent` EIP-712 type and the `Sha256Proxy` digest. The copies are
  marked `#[inline(never)]`, as a non-generic function of another crate is not inlined. Because
  the wire order type is not exported, an order is made by cloning a template order read once
  into `Actions` and setting its fields, which adds a few small allocations to build + sign +
  body.
- The signing step is therefore a copy of its private code. The harness's `verify-copy` checks
  the copy byte for byte against the SDK itself: the SDK's public `ExchangeClient::order` posts
  the fixed order to a local server on `localhost:3001` (its `BaseUrl::Localhost`), and the copy
  must build, hash, sign and serialize the same order at the nonce the SDK chose into the same
  body bytes. Nothing reaches the venue. Every compare run records the result under
  `copy_check`, and the signing table's note states it. A public call from Hyperliquid that
  signs an order without sending it, with a given nonce, is welcome; the harness would then
  time that call instead of the copy.
- Parse times what `WsManager` does with each text frame before sending it on:
  `serde_json::from_str::<Message>` and the subscription key it builds for the message.
- It has no `bbo` subscription and no `fast` option for l2Book.

@nktkas/hyperliquid 0.33.3 on Node (`ts/`):

- Signing uses the calls its signing documentation shows: `canonicalize` with the order schema,
  `signL1Action` with a viem account (viem is pinned in `ts/pnpm-lock.yaml`) and
  `JSON.stringify` of the body. `ExchangeClient.order` also validates the order with valibot
  before signing; that step is not timed. `signL1Action` is async, and every timed call awaits
  it.
- Parse times the SDK's own `HyperliquidEventTarget`: `JSON.parse`, the envelope check and a
  `CustomEvent` dispatched to a listener on the channel. The class is not in the package's
  exports, so the harness loads it from its file. One reused `message` event stands in for the
  socket's, so the runtime's own cost of making that event is not counted. Node cannot count
  allocations, so that column shows "-".
- Node has no call to ask macOS for performance cores, so these rows run without the
  quality-of-service request the others make.
- Round trips use an undici `Agent` with a 90 s keep-alive, passed through `HttpTransport`'s
  `fetchOptions` (undici 6.21.1, the version Node 22.14 bundles). Node's fetch closes a
  connection after 4 s idle when the server sends no keep-alive timeout, and
  api.hyperliquid.xyz sends none. With every SDK taking its turn, each SDK's requests are about
  5 s apart, so with Node's default every request opened a new TLS connection, while the Rust,
  Go and Python clients keep an idle connection for 90 s or more. Results files up to
  `2026-11-27-215626` were made before this change and have the slower figures for that reason.

go-hyperliquid 0.47.3 (`go/`):

- Its public `SignL1Action` takes the nonce, so it signs the fixed order directly. The order
  action is built with its public wire types as its private `newCreateOrderAction` builds it,
  with a copy of its private `floatToWire`.
- go-ethereum chooses its secp256k1 implementation at build time. With cgo on, which is Go's
  default when a C compiler is found, it signs with libsecp256k1 through cgo, the same C library
  as the `df-hyperliquid-core + secp256k1` row's. With cgo off it signs with
  `github.com/decred/dcrd/dcrec/secp256k1/v4`, which is written in Go. The harness has the same
  build constraints in `secp_cgo.go` and `secp_nocgo.go`, and every results file records which
  one the binary was built with, under `versions.go_build` and on the "Go build" line of the
  `.md` and `.pdf`.
- Parse times what its websocket client does with each frame: the envelope through the
  easyjson decoder generated for its private `wsMessage` (copied), then `json.Unmarshal` of the
  data into `L2Book` or `Trades`. Book prices and sizes are read into `float64`. Allocations are
  Go's heap allocation count.
- It writes r and s without leading zeros in the body; the signature check compares them padded
  to 32 bytes.
- Its client logs a frame it cannot read only in debug mode, which logs every frame, so "not
  read" is not counted. Go has no call to ask macOS for performance cores without cgo, so these
  rows run without the quality-of-service request.

The harness for each competitor is in `hypersdk/`, `lhermoso/`, `quicknode/`, `official-rust/`,
`ts/`, `go/` and `py/`. If an SDK has a faster documented
way to do any of these steps, a pull request that uses it is welcome, and the next published
results will include it.

## Platform effects

- Keccak runs on the ARMv8.2 SHA3 instructions on Apple silicon and other ARMv8.2 CPUs, and on
  the portable path elsewhere, including x86-64. The signing gap may differ on x86-64. The
  published results are from Apple silicon; results from other machines are welcome (below).
- Laptops on battery, low-power modes and other load change the figures. The runner records the
  power source and energy mode (`pmset` on macOS, `/sys/class/power_supply` and the ACPI platform
  profile on Linux) and prints a warning on battery or in low power mode; the warning is also in
  the `.md` and `.pdf`, and `bench-share` repeats it. The offline tests run one process at a time.
- On macOS every benchmark program asks for the user-interactive quality of service
  (`pthread_set_qos_class_self_np`) on each thread that does timed work: the main thread, every
  tokio worker, and the Python main thread. The scheduler then keeps them on performance cores
  more reliably, which makes laptop results steadier. macOS does not pass the class on to
  threads started later, so each thread asks for itself.
- The Python figures depend on the CPython version, which `py/uv.lock` pins (3.13).
- Code generation changes some signing rows. With cargo's default release profile (16 codegen
  units), hypersdk's hash + sign took 33 to 35 us in these runs; built with
  `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, the same harness took 25.0 us, level with
  df-hyperliquid-core (25.5 us built the same way, 25.2 us with the default profile).
  hyperliquid_rust_sdk moved between 24.5 and 31 us across builds of its harness that changed
  only untimed code. The published rows use the default profile, which is what a user's
  `cargo build --release` gives, but a signing difference of this size between k256-based rows
  is within what code generation alone can change.

## Method

- Signing: 2,000 warm-up calls, then 20,000 timed calls, each timed on its own, for the Rust,
  TypeScript and Go rows. Python: 200 warm-up calls, then 2,000 timed calls (pure Python) or
  5,000 (coincurve).
- Parse: 3 untimed passes over the corpus, then 20 timed passes; frames per second are over the
  timed passes.
- Each offline test runs 5 times as a separate process. The run with the median result is
  reported, and every run's figure is kept in the JSON under `runs`.
- `--quick`: a tenth of the signing samples, a quarter of the parse passes and one run.

## Results files

Every run writes `results/<date>-<hhmmss>-<cpu>.json`, `.md` and `.pdf`; `-df` marks a
df-hyperliquid-core run alone. Each records the CPU, cores, memory, OS, architecture, power source
and energy mode (never the host or user name), the rustc version, df-hyperliquid-core's version and git commit, the
competitors' pinned versions, the options, every run's figures, every raw network sample, what
was skipped, and the build and benchmark times. The `.pdf` is the same tables laid out to read
and share.

Competitor versions are pinned: hypersdk through `Cargo.lock`, hyperliquid-sdk-rs 0.1.2,
quicknode-hyperliquid-sdk 0.1.9 and hyperliquid_rust_sdk 0.6.0 through the `Cargo.lock` in their
own folders (every Rust build uses `--locked`), @nktkas/hyperliquid 0.33.3 and viem through
`ts/pnpm-lock.yaml` (`--frozen-lockfile`), go-hyperliquid v0.47.3 through `go/go.mod` and
`go/go.sum`, and the Python SDK and its dependencies through `py/uv.lock`. hypersdk stays at
0.3.1 although 0.3.2 is out: versions are pinned, and moving one is a separate change.

## Results

From `results/2026-11-27-215626-apple-m5-max.json` (also `.md` and `.pdf`), one
`just bench-compare --network` run on an Apple M5 Max on mains power, in automatic energy mode,
against seven other SDK rows.

- Websocket parsing: df-hyperliquid-core read the recorded frames 2.4 times as fast as hypersdk,
  the next fastest, and 3.3 times as fast as hyperliquid-sdk-rs with simd-json, with the lowest
  p99 per frame, and the fewest allocations of the rows whose runtime counts them. No competitor
  was faster on any parse figure.
- Signing with the default build (k256): go-hyperliquid was faster than df-hyperliquid-core,
  15.6 us against 25.2 us for hash + sign, because go-ethereum signs through libsecp256k1.
  hyperliquid_rust_sdk was level with df-hyperliquid-core (24.5 us against 25.2 us for
  hash + sign, 25.5 us against 26.7 us for build + sign + body). hypersdk, hyperliquid-sdk-rs
  and quicknode-hyperliquid-sdk took 33 to 34 us.
- Signing with the `secp256k1` feature: df-hyperliquid-core was the fastest row, 13.0 us against
  go-hyperliquid's 15.6 us.
- The hypersdk signing gap depends on code generation; see Platform effects.
- Round trips and the live websocket were level for the SDKs that talk to the venue directly.
  quicknode-hyperliquid-sdk's round trips go through QuickNode's worker and took longer; its
  live row received nothing (see its fairness notes). hyperliquid-sdk-rs and
  hyperliquid_rust_sdk got 5 normal books in 60 s because they cannot ask for the fast book.

These are Apple silicon figures; see Platform effects.

Machine: Apple M5 Max, 18 cores, 128 GB, macOS 26.5.2, aarch64  
Versions: df-hyperliquid-core 0.2.5 (commit c70c176-dirty), hypersdk 0.3.1, hyperliquid-sdk-rs 0.1.2, quicknode-hyperliquid-sdk 0.1.9, hyperliquid_rust_sdk 0.6.0, @nktkas/hyperliquid 0.33.3, go-hyperliquid 0.47.3, hyperliquid-python-sdk 0.24.0; rustc 1.94.1 (e408947bf 2026-03-25)  
Run: compare, offline and network, 5 runs per row with the median run reported, started 2026-11-27T21:56:26Z  
Power: AC Power, automatic mode  

### Signing one BTC limit order (offline)

| SDK | hash + sign median (us) | p99 (us) | build + sign + body median (us) | p99 (us) |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 25.2 | 27.2 | 26.7 | 29.0 |
| df-hyperliquid-core + secp256k1 | **13.0** | 13.4 | 14.4 | 15.1 |
| hypersdk | 33.4 | 36.2 | 33.9 | 36.3 |
| hyperliquid-sdk-rs | 33.8 | 36.6 | 35.8 | 38.7 |
| quicknode-hyperliquid-sdk | 33.2 | 36.5 | 35.3 | 38.4 |
| hyperliquid_rust_sdk | 24.5 | 26.6 | 25.5 | 27.6 |
| @nktkas/hyperliquid | 172.5 | 266.7 | 174.2 | 267.9 |
| go-hyperliquid | 15.6 | 19.0 | 16.8 | 20.1 |
| hyperliquid-python-sdk | 2015.8 | 2110.7 | 2009.1 | 2049.7 |
| hyperliquid-python-sdk + coincurve | 123.8 | 147.2 | 130.0 | 152.4 |

Same signature from every row: yes. hypersdk's sign_sync takes the action by value, so its timed call signs a clone; the clone alone takes 41 ns at the median.

### Recovering the signer from a signature (offline)

| SDK | median (us) | p99 (us) |
|---|---:|---:|
| df-hyperliquid-core | 81.1 | 85.4 |
| df-hyperliquid-core + secp256k1 | **14.8** | 15.1 |

Client::send recovers a wallet's signature before posting; a local key's is not recovered in a release build.

### Websocket parse (4,000 recorded mainnet frames, offline)

| SDK | all frames/s | l2Book frames/s | trades frames/s | p99 per frame (ns) | allocations per frame | not read |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | **1,119,244** | 798,732 | 1,972,966 | 2,875 | 4.7 | 0 |
| hypersdk | 475,581 | 296,855 | 1,267,841 | 4,667 | 7.7 | 0 |
| hyperliquid-sdk-rs | 336,676 | 214,177 | 851,604 | 5,750 | 85.1 | 0 |
| quicknode-hyperliquid-sdk | 197,184 | 122,933 | 521,949 | 11,083 | 182.0 | 0 |
| hyperliquid_rust_sdk | 297,227 | 185,787 | 875,772 | 6,458 | 81.6 | 0 |
| @nktkas/hyperliquid | 291,390 | 177,916 | 836,046 | 7,125 | - | 0 |
| go-hyperliquid | 74,846 | 51,820 | 142,275 | 46,625 | 29.6 | 0 |
| hyperliquid-python-sdk | 226,977 | 148,063 | 482,606 | 9,709 | - | 0 |

What each row reads a frame into: df-hyperliquid-core: typed values with exact decimals; hypersdk: typed values with exact decimals; hyperliquid-sdk-rs: typed values with strings, with simd-json; quicknode-hyperliquid-sdk: an untyped serde_json::Value; hyperliquid_rust_sdk: typed values with strings; @nktkas/hyperliquid: JSON.parse objects, dispatched as events; go-hyperliquid: typed values with float64 book levels; hyperliquid-python-sdk: dicts of strings. Allocations are counted where the runtime can count them.

### Info round trip: POST /info l2Book BTC (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | failed |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 332.9 | 493.8 | 1653.3 | 0 |
| hypersdk | 352.0 | 522.7 | 1373.0 | 0 |
| hyperliquid-sdk-rs | 325.5 | 551.9 | 1523.8 | 0 |
| quicknode-hyperliquid-sdk | 721.3 | 911.3 | 1848.6 | 0 |
| hyperliquid_rust_sdk | 329.6 | 415.3 | 823.9 | 0 |
| @nktkas/hyperliquid | 412.8 | 615.6 | 3353.5 | 0 |
| go-hyperliquid | 356.6 | 516.6 | 1096.0 | 0 |
| hyperliquid-python-sdk | 324.3 | 493.6 | 1174.5 | 0 |

Almost all of a round trip is the network and the venue, so these show parity, not a ranking.

### Exchange round trip: refused post-only order from an empty key (100 requests per SDK)

| SDK | median (ms) | p90 (ms) | p99 (ms) | failed |
|---|---:|---:|---:|---:|
| df-hyperliquid-core | 363.9 | 513.7 | 930.2 | 0 |
| hypersdk | 362.8 | 556.8 | 1242.8 | 0 |
| hyperliquid-sdk-rs | 360.1 | 517.9 | 1403.3 | 0 |
| quicknode-hyperliquid-sdk | 1133.7 | 1549.3 | 4030.3 | 0 |
| hyperliquid_rust_sdk | 341.5 | 458.0 | 1467.1 | 0 |
| @nktkas/hyperliquid | 454.8 | 701.5 | 5082.8 | 0 |
| go-hyperliquid | 374.2 | 488.1 | 797.8 | 0 |
| hyperliquid-python-sdk | 348.7 | 465.7 | 816.3 | 0 |

Almost all of a round trip is the network and the venue, so these show parity, not a ranking.

### Live websocket, BTC l2Book (fast), bbo and trades, 60 s, every SDK at once

| SDK | l2Book | bbo | trades frames | not read | age median (ms) | age p99 (ms) |
|---|---:|---:|---:|---:|---:|---:|
| df-hyperliquid-core | 108 | 394 | 66 | 0 | 269 | 635 |
| hypersdk | 108 | 397 | 66 | 0 | 263 | 626 |
| hyperliquid-sdk-rs | 5 | 394 | 66 | - | 290 | 725 |
| quicknode-hyperliquid-sdk | 0 | 0 | 0 | - | 0 | 0 |
| hyperliquid_rust_sdk | 5 | - | 66 | 0 | 284 | 757 |
| @nktkas/hyperliquid | 109 | 392 | 65 | - | 264 | 598 |
| go-hyperliquid | 108 | 395 | 66 | - | 257 | 598 |
| hyperliquid-python-sdk | 108 | 392 | 66 | - | 281 | 667 |

Age is the local clock minus the venue's timestamp; it compares the SDKs with each other. hyperliquid-sdk-rs, quicknode-hyperliquid-sdk, hyperliquid_rust_sdk have no fast l2Book option, so they get the normal book, sent less often. hyperliquid_rust_sdk has no bbo subscription. quicknode-hyperliquid-sdk's Stream documents a QuickNode endpoint; without one it sends its subscriptions to the public websocket, which refuses them ("Error parsing JSON into valid websocket request: {"method":"..."), so it receives no market data.

## Submit your results

Run `just bench-compare` on mains power and outside low power mode, then open an issue with the
[benchmark result template](https://github.com/dennisfurrer/df-hyperliquid-core/issues/new?template=benchmark-result.yml)
and paste the JSON file from `bench/results/`. `just bench-share` prints a line to post.

## Recording a new corpus

```sh
cd bench/py && uv run python record.py --out ../data/frames.jsonl.gz --per-channel 2000 --books 150
cd bench/py && uv run python record.py --coverage --out ../data/coverage.jsonl.gz --per-channel 500 --books 20
```

## Files

- `runner/`: the runner the commands above start: builds, runs, writes the results files.
- `common/`: what the Rust benchmark binaries share: the fixed signing input, the timing harness
  and the corpus loader.
- `df/`, `hypersdk/`: the benchmark binaries of df-hyperliquid-core and hypersdk, in this
  workspace.
- `lhermoso/`, `quicknode/`, `official-rust/`: the benchmark binaries of hyperliquid-sdk-rs,
  quicknode-hyperliquid-sdk and hyperliquid_rust_sdk, each its own workspace with its own
  `Cargo.lock`, because their alloy and ethers versions cannot share one dependency graph.
- `ts/`: the @nktkas/hyperliquid benchmark (`bench.mjs`) and its pnpm lockfile.
- `go/`: the go-hyperliquid benchmark (`main.go`) and its `go.mod` and `go.sum`.
- `py/`: the Python SDK benchmark (`bench_py.py`), the corpus recorder (`record.py`) and the
  pinned uv project.
- `data/`: the recorded websocket corpora.
- `results/`: one set of files per run. `2026-11-24.*` and `2026-11-27.*` are from the earlier
  `run.py` harness.
