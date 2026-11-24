#!/usr/bin/env bash
# Runs every benchmark and writes results/<date>.json and results/<date>.md.
# Needs: rustup (the toolchain in ../rust-toolchain.toml), uv, and network access to
# api.hyperliquid.xyz. Set BENCH_LOCATION (city, country) to record where it ran.
set -euo pipefail
cd "$(dirname "$0")"

# One binary at a time, so cargo does not unify the two SDKs' dependency features.
cargo build --release --locked -p bench-df
cargo build --release --locked -p bench-hypersdk

uv sync --project py --frozen

uv run --project py --frozen python run.py
