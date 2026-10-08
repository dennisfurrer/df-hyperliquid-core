# The gate before any commit.
check: fmt clippy wasm test features

fmt:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Every library builds for the browser and Workers.
wasm:
    cargo clippy --workspace --target wasm32-unknown-unknown -- -D warnings

test:
    cargo test --workspace

# Signing is a feature: hl-core builds without it, and no crate pulls the curve in unless asked.
features:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy -p hl-core --no-default-features -- -D warnings
    cargo clippy -p hl-core --no-default-features --target wasm32-unknown-unknown -- -D warnings
    for crate in hl-account hl-transfer hl-trade hl-perps hl-spot hl-hip3 hl-hip4 hl-lend hl-stake hl-vault; do
        if cargo tree -p "$crate" -e no-dev --target all | grep -q ' k256 v'; then
            echo "$crate pulls in k256 without asking to sign" >&2
            exit 1
        fi
    done
