//! Building the benchmark binaries, each on its own so cargo does not merge the SDKs'
//! dependency features: each binary is what a program using only that SDK would compile. The
//! Rust competitors other than hypersdk are separate workspaces with their own Cargo.lock
//! (`lhermoso/`, `quicknode/`, `official-rust/`), because their alloy and ethers versions cannot
//! share one dependency graph. The TypeScript SDK is installed with pnpm (`ts/`) and the Go SDK
//! built with go (`go/`); each is skipped with a note when its tool is missing.
//!
//! Rust binaries are built with the profile `--profile` names: `release` (cargo's default release
//! profile), `release-lto` (fat link-time optimization and one codegen unit, defined in each
//! workspace's Cargo.toml) or both, each into `target/.../<profile>/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::offline::{GO, LHERMOSO, NKTKAS, OFFICIAL_RUST, QUICKNODE};
use crate::tools::Tools;
use crate::{Options, Result, progress};

/// Cargo's default release profile.
pub const RELEASE: &str = "release";
/// Fat link-time optimization and one codegen unit.
pub const RELEASE_LTO: &str = "release-lto";

/// What was built. A row whose tool is missing is `None` or `false`, with a note in `skipped`.
/// `profile` is the Rust profile of the Rust binaries; `other` holds the Rust binaries of the
/// second profile when both were asked for.
pub struct Binaries {
    pub profile: &'static str,
    pub other: Option<Box<Binaries>>,
    pub df: PathBuf,
    pub df_secp: Option<PathBuf>,
    pub hypersdk: Option<PathBuf>,
    pub lhermoso: Option<PathBuf>,
    pub quicknode: Option<PathBuf>,
    pub official_rust: Option<PathBuf>,
    pub ts: bool,
    pub go: Option<PathBuf>,
    pub python: bool,
}

fn cargo(bench: &Path, profile: &str, args: &[&str]) -> bool {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Command::new(cargo)
        .args(["build", "--profile", profile, "--locked", "--quiet"])
        .args(args)
        .current_dir(bench)
        .status()
        .is_ok_and(|status| status.success())
}

/// A Rust competitor in its own workspace, `<folder>/Cargo.toml` with its own Cargo.lock, built
/// into `target/<folder>/<profile>`.
fn standalone(bench: &Path, profile: &str, folder: &str, name: &str, skipped: &mut Vec<String>) -> Option<PathBuf> {
    progress(&format!("building {name} ({profile})"));
    let manifest = format!("{folder}/Cargo.toml");
    let target = format!("target/{folder}");
    if cargo(bench, profile, &["--manifest-path", &manifest, "--target-dir", &target]) {
        Some(bench.join(format!("target/{folder}/{profile}/bench-{folder}")))
    } else {
        skipped.push(format!("{name} ({profile}): the build failed; see the cargo output"));
        None
    }
}

/// Nothing built yet, for `profile`.
fn empty(profile: &'static str) -> Binaries {
    Binaries {
        profile,
        other: None,
        df: PathBuf::new(),
        df_secp: None,
        hypersdk: None,
        lhermoso: None,
        quicknode: None,
        official_rust: None,
        ts: false,
        go: None,
        python: false,
    }
}

/// Every Rust binary in one profile. `note_missing` adds the notes for missing tools, which the
/// second profile's build leaves out so they are not listed twice.
fn rust(
    bench: &Path,
    profile: &'static str,
    options: &Options,
    found: &Tools,
    note_missing: bool,
    skipped: &mut Vec<String>,
) -> Result<Binaries> {
    let mut built = empty(profile);
    progress(&format!("building df-hyperliquid-core ({profile}; the first build takes a few minutes)"));
    if !cargo(bench, profile, &["-p", "bench-df"]) {
        return Err(format!("building bench-df ({profile}) failed; see the cargo output above"));
    }
    built.df = bench.join(format!("target/{profile}/bench-df"));

    match &found.c_compiler {
        Some(_) => {
            progress(&format!("building df-hyperliquid-core with the secp256k1 feature ({profile})"));
            if cargo(bench, profile, &["-p", "bench-df", "--features", "secp256k1", "--target-dir", "target/secp256k1"])
            {
                built.df_secp = Some(bench.join(format!("target/secp256k1/{profile}/bench-df")));
            } else {
                skipped
                    .push(format!("df-hyperliquid-core + secp256k1 ({profile}): the build failed (libsecp256k1 is C)"));
            }
        }
        None if note_missing => skipped.push("df-hyperliquid-core + secp256k1: no C compiler found".to_owned()),
        None => {}
    }

    if options.compare {
        progress(&format!("building hypersdk ({profile})"));
        if !cargo(bench, profile, &["-p", "bench-hypersdk"]) {
            return Err(format!("building bench-hypersdk ({profile}) failed; see the cargo output above"));
        }
        built.hypersdk = Some(bench.join(format!("target/{profile}/bench-hypersdk")));
        built.lhermoso = standalone(bench, profile, "lhermoso", LHERMOSO, skipped);
        if found.protoc.is_some() || std::env::var_os("PROTOC").is_some() {
            built.quicknode = standalone(bench, profile, "quicknode", QUICKNODE, skipped);
        } else if note_missing {
            skipped.push(format!("{QUICKNODE}: protoc not found (its build script compiles protobuf files)"));
        }
        built.official_rust = standalone(bench, profile, "official-rust", OFFICIAL_RUST, skipped);
    }
    Ok(built)
}

/// `node --version` (`v22.14.0`) as numbers.
fn node_numbers(version: &str) -> Vec<u64> {
    version.trim_start_matches('v').split('.').filter_map(|part| part.parse().ok()).collect()
}

/// The TypeScript and Go rows, added to `built`.
fn others(bench: &Path, found: &Tools, built: &mut Binaries, skipped: &mut Vec<String>) {
    match (&found.node, &found.pnpm) {
        (Some(node), Some(_)) if node_numbers(node) >= vec![22, 12] => {
            progress("installing @nktkas/hyperliquid with pnpm");
            let installed = Command::new("pnpm")
                .args(["install", "--frozen-lockfile", "--silent"])
                .current_dir(bench.join("ts"))
                .status()
                .is_ok_and(|status| status.success());
            if installed {
                built.ts = true;
            } else {
                skipped.push(format!("{NKTKAS}: `pnpm install --frozen-lockfile` failed"));
            }
        }
        (Some(node), Some(_)) => skipped.push(format!("{NKTKAS}: node {node} is older than 22.12")),
        (None, _) => skipped.push(format!("{NKTKAS}: node not found")),
        (_, None) => skipped.push(format!("{NKTKAS}: pnpm not found")),
    }

    match &found.go {
        Some(_) => {
            progress("building go-hyperliquid");
            let built_go = Command::new("go")
                .args(["build", "-o", "../target/go/bench-go", "."])
                .current_dir(bench.join("go"))
                .status()
                .is_ok_and(|status| status.success());
            if built_go {
                built.go = Some(bench.join("target/go/bench-go"));
            } else {
                skipped.push(format!("{GO}: `go build` failed"));
            }
        }
        None => skipped.push(format!("{GO}: go not found")),
    }
}

pub fn build(bench: &Path, options: &Options, found: &Tools, skipped: &mut Vec<String>) -> Result<Binaries> {
    let mut built = rust(bench, options.profiles[0], options, found, true, skipped)?;
    if options.compare {
        others(bench, found, &mut built, skipped);
        match &found.uv {
            Some(_) => {
                progress("setting up the Python SDK with uv");
                let synced = Command::new("uv")
                    .args(["sync", "--project", "py", "--frozen", "--quiet"])
                    .current_dir(bench)
                    .status()
                    .is_ok_and(|status| status.success());
                if synced {
                    built.python = true;
                } else {
                    skipped.push("hyperliquid-python-sdk: `uv sync` failed".to_owned());
                }
            }
            None => skipped.push("hyperliquid-python-sdk: uv not found".to_owned()),
        }
    }
    if let Some(second) = options.profiles.get(1) {
        built.other = Some(Box::new(rust(bench, second, options, found, false, skipped)?));
    }
    Ok(built)
}
