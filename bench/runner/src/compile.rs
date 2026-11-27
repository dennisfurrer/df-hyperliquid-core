//! Building the benchmark binaries, each on its own so cargo does not merge the SDKs'
//! dependency features: each binary is what a program using only that SDK would compile.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::tools::Tools;
use crate::{Options, Result, progress};

/// What was built. A row whose tool is missing is `None` or `false`, with a note in `skipped`.
pub struct Binaries {
    pub df: PathBuf,
    pub df_secp: Option<PathBuf>,
    pub hypersdk: Option<PathBuf>,
    pub python: bool,
}

fn cargo(bench: &Path, args: &[&str]) -> bool {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Command::new(cargo)
        .args(["build", "--release", "--locked", "--quiet"])
        .args(args)
        .current_dir(bench)
        .status()
        .is_ok_and(|status| status.success())
}

pub fn build(bench: &Path, options: &Options, found: &Tools, skipped: &mut Vec<String>) -> Result<Binaries> {
    progress("building df-hyperliquid-core (the first build takes a few minutes)");
    if !cargo(bench, &["-p", "bench-df"]) {
        return Err("building bench-df failed; see the cargo output above".to_owned());
    }
    let df = bench.join("target/release/bench-df");

    let df_secp = match &found.c_compiler {
        Some(_) => {
            progress("building df-hyperliquid-core with the secp256k1 feature");
            if cargo(bench, &["-p", "bench-df", "--features", "secp256k1", "--target-dir", "target/secp256k1"]) {
                Some(bench.join("target/secp256k1/release/bench-df"))
            } else {
                skipped.push("df-hyperliquid-core + secp256k1: the build failed (libsecp256k1 is C)".to_owned());
                None
            }
        }
        None => {
            skipped.push("df-hyperliquid-core + secp256k1: no C compiler found".to_owned());
            None
        }
    };

    let mut hypersdk = None;
    let mut python = false;
    if options.compare {
        progress("building hypersdk");
        if !cargo(bench, &["-p", "bench-hypersdk"]) {
            return Err("building bench-hypersdk failed; see the cargo output above".to_owned());
        }
        hypersdk = Some(bench.join("target/release/bench-hypersdk"));
        match &found.uv {
            Some(_) => {
                progress("setting up the Python SDK with uv");
                let synced = Command::new("uv")
                    .args(["sync", "--project", "py", "--frozen", "--quiet"])
                    .current_dir(bench)
                    .status()
                    .is_ok_and(|status| status.success());
                if synced {
                    python = true;
                } else {
                    skipped.push("hyperliquid-python-sdk: `uv sync` failed".to_owned());
                }
            }
            None => skipped.push("hyperliquid-python-sdk: uv not found".to_owned()),
        }
    }
    Ok(Binaries { df, df_secp, hypersdk, python })
}
