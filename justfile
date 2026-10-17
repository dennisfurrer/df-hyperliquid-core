check: fmt clippy wasm test web features doc

fmt:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --features web -- -D warnings

wasm:
    cargo clippy -p df-hyperliquid-core --no-default-features --features sign,web --target wasm32-unknown-unknown -- -D warnings

test:
    cargo test --workspace

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

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
