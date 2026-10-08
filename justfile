# The gate before any commit.
check: fmt clippy wasm test web features

fmt:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p hl --all-targets --features native,sign -- -D warnings

# Every library builds for the browser and Workers, but the native wire.
wasm:
    cargo clippy --workspace --exclude hl-native --target wasm32-unknown-unknown -- -D warnings

test:
    cargo test --workspace

# The browser wire, run as wasm in Node through wasm-bindgen's test runner (.cargo/config.toml).
web:
    cargo clippy -p hl-web --target wasm32-unknown-unknown --all-targets -- -D warnings
    cargo test -p hl-web --target wasm32-unknown-unknown

# Signing is a feature: hl-core builds without it, and no crate pulls the curve in unless asked.
features:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy -p hl-core --no-default-features -- -D warnings
    cargo clippy -p hl-core --no-default-features --target wasm32-unknown-unknown -- -D warnings
    for crate in hl-account hl-transfer hl-trade hl-perps hl-spot hl-hip3 hl-hip4 hl-lend hl-stake hl-vault hl-native hl-web hl-hydromancer hl-ws hl-evm hl-store hl-analytics hl; do
        if cargo tree -p "$crate" -e no-dev --target all | grep -q ' k256 v'; then
            echo "$crate pulls in k256 without asking to sign" >&2
            exit 1
        fi
    done
