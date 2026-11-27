check: fmt clippy wasm test web features doc

fmt:
    cargo fmt --all --check

# Every configuration whose code differs: default with the browser transport and the live suite's
# hook, and the libsecp256k1 backend.
clippy:
    cargo clippy --workspace --all-targets --features web,__test-internals -- -D warnings
    cargo clippy --workspace --all-targets --features secp256k1 -- -D warnings

wasm:
    cargo clippy -p df-hyperliquid-core --no-default-features --features sign,web --target wasm32-unknown-unknown -- -D warnings
    # `secp256k1` changes nothing on wasm, which keeps k256.
    cargo clippy -p df-hyperliquid-core --no-default-features --features sign,web,secp256k1 --target wasm32-unknown-unknown -- -D warnings

# Unit, conformance and mocked suites, and the direct frame reader against the recorded mainnet
# corpus in bench/data; the live suite is skipped here.
test:
    cargo test --workspace
    cargo test -p df-hyperliquid-core --features offline --lib -- offline what_is_signed
    cargo test -p df-hyperliquid-core --lib -- --ignored mainnet_corpus
    cargo test -p df-hyperliquid-core --features secp256k1 --lib

# Against mainnet, read only: every read, websocket channel and HyperEVM read, each held
# against its recording. Run by hand.
live:
    cargo test -p df-hyperliquid-core --features __test-internals --test live -- --ignored --test-threads 1

# Against testnet: every L1 action in vectors/ signed by a new key with no account and posted
# to /exchange. The node must read each one and recover the key's address. Nothing can execute.
testnet:
    cargo test -p df-hyperliquid-core --test testnet -- --ignored --nocapture

# Line coverage of the unit, conformance and mocked suites.
cover:
    cargo llvm-cov -p df-hyperliquid-core --summary-only

# The browser transport, run as wasm in Node through wasm-bindgen's test runner.
web:
    cargo test -p df-hyperliquid-core --no-default-features --features sign,web --target wasm32-unknown-unknown --lib

# Without `sign`, nothing pulls in the curve.
features:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy -p df-hyperliquid-core --no-default-features -- -D warnings
    cargo clippy -p df-hyperliquid-core --no-default-features --target wasm32-unknown-unknown -- -D warnings
    if cargo tree -p df-hyperliquid-core --no-default-features -e no-dev --target all | grep -q ' k256 v'; then
        echo "df-hyperliquid-core pulls in k256 without sign" >&2
        exit 1
    fi
    if cargo tree -p df-hyperliquid-core -e no-dev --target all | grep -q ' secp256k1 v'; then
        echo "df-hyperliquid-core pulls in libsecp256k1 without the secp256k1 feature" >&2
        exit 1
    fi

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

# Fuzzes one target for `secs` seconds (nightly, cargo-fuzz): `decode` (live::decode against the
# two-pass reader, seeded with the recorded frames), `dec` (the two decimal readers) or `backends`
# (k256 against libsecp256k1, signing and recovery).
fuzz target="decode" secs="300":
    #!/usr/bin/env bash
    set -euo pipefail
    cd packages/core/fuzz
    mkdir -p corpus/{{target}}
    if [ "{{target}}" = decode ]; then
        for name in frames coverage; do
            gzip -dc ../../../bench/data/$name.jsonl.gz | awk 'NR % 20 == 1' | split -l 1 - corpus/decode/$name-
        done
        python3 -c 'import json,glob,hashlib
    for path in glob.glob("../fixtures/ws/*.json"):
        for text in json.load(open(path)).get("frames", {}).values():
            name = "corpus/decode/fixture-" + hashlib.sha1(text.encode()).hexdigest()[:16]
            open(name, "w").write(text)'
    fi
    dict=""
    if [ -f {{target}}.dict ]; then dict="-dict={{target}}.dict"; fi
    cargo +nightly fuzz run {{target}} corpus/{{target}} -- $dict -max_total_time={{secs}} -max_len=65536
