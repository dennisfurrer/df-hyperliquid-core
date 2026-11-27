//! The benchmark runner. It builds the benchmark binaries, runs them and writes
//! `results/<date>-<cpu>.json` and `.md`. Commands:
//!
//! - `run`: df-hyperliquid-core alone, offline. `--compare` adds the other SDKs (hypersdk,
//!   hyperliquid-sdk-rs, quicknode-hyperliquid-sdk, hyperliquid_rust_sdk, @nktkas/hyperliquid,
//!   go-hyperliquid and the Python SDK),
//!   `--network` adds the round trips and the live websocket, `--quick` checks that everything
//!   runs instead of measuring, `--profile release|release-lto|both` picks the Rust build profile.
//! - `doctor`: which tools are installed and what each one enables.
//! - `share [results.json]`: a summary of at most 280 characters to post.

mod compile;
mod machine;
mod network;
mod offline;
mod pdf;
mod pdf_report;
mod report;
mod tools;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use serde_json::{Value, json};

pub type Result<T> = std::result::Result<T, String>;

const USAGE: &str = "usage: bench-runner run [--compare] [--network] [--quick] [--profile release|release-lto|both]
                        [--runs N] [--rounds N] [--live-secs N]
       bench-runner doctor
       bench-runner share [results.json]";

/// What `run` was asked to do.
#[derive(Clone, Debug)]
pub struct Options {
    pub compare: bool,
    pub network: bool,
    pub quick: bool,
    /// Runs of each offline benchmark; the run with the median result is reported.
    pub runs: usize,
    /// Paced network rounds, after 3 warm-up rounds.
    pub rounds: usize,
    pub live_secs: u64,
    /// The Rust build profiles: `release`, `release-lto`, or both in that order. The first is the
    /// one every table ranks; the second adds columns beside it.
    pub profiles: Vec<&'static str>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Options> {
        let mut options = Options {
            compare: false,
            network: false,
            quick: false,
            runs: 5,
            rounds: 100,
            live_secs: 60,
            profiles: vec![compile::RELEASE],
        };
        let mut given = (None, None, None);
        let mut at = 0;
        while at < args.len() {
            let number = |at: usize| -> Result<u64> {
                let text = args.get(at + 1).ok_or_else(|| format!("{} needs a number", args[at]))?;
                text.parse().map_err(|_| format!("{} needs a number, not {text}", args[at]))
            };
            match args[at].as_str() {
                "--compare" => options.compare = true,
                "--network" => options.network = true,
                "--quick" => options.quick = true,
                "--runs" => (given.0, at) = (Some(number(at)? as usize), at + 1),
                "--rounds" => (given.1, at) = (Some(number(at)? as usize), at + 1),
                "--live-secs" => (given.2, at) = (Some(number(at)?), at + 1),
                "--profile" => {
                    options.profiles = match args.get(at + 1).map(String::as_str) {
                        Some("release") => vec![compile::RELEASE],
                        Some("release-lto") => vec![compile::RELEASE_LTO],
                        Some("both") => vec![compile::RELEASE, compile::RELEASE_LTO],
                        _ => return Err(format!("--profile needs release, release-lto or both\n{USAGE}")),
                    };
                    at += 1;
                }
                other => return Err(format!("unknown option {other}\n{USAGE}")),
            }
            at += 1;
        }
        if options.quick {
            (options.runs, options.rounds, options.live_secs) = (1, 10, 15);
        }
        options.runs = given.0.unwrap_or(options.runs).max(1);
        options.rounds = given.1.unwrap_or(options.rounds).max(1);
        options.live_secs = given.2.unwrap_or(options.live_secs).max(5);
        Ok(options)
    }
}

/// The `bench/` directory, wherever the runner is started from.
pub fn bench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// One progress line on stderr, with the time since the runner started.
pub fn progress(step: &str) {
    static START: OnceLock<Instant> = OnceLock::new();
    let seconds = START.get_or_init(Instant::now).elapsed().as_secs();
    eprintln!("[{:02}:{:02}] {step}", seconds / 60, seconds % 60);
}

fn main() {
    progress("bench-runner");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("run") => Options::parse(&args[1..]).and_then(|options| run(&options)),
        Some("doctor") => {
            tools::doctor(&bench_dir());
            Ok(())
        }
        Some("share") => report::share(&bench_dir(), args.get(1).map(PathBuf::from)),
        _ => Err(USAGE.to_owned()),
    };
    if let Err(error) = outcome {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(options: &Options) -> Result<()> {
    let bench = bench_dir();
    let found = tools::find(&bench);
    let mut skipped = Vec::new();
    let building = Instant::now();
    let binaries = compile::build(&bench, options, &found, &mut skipped)?;
    let build_seconds = building.elapsed().as_secs_f64();
    let measuring = Instant::now();
    let machine = machine::describe();
    let versions = report::versions(&bench, &binaries);
    let cpu = machine["cpu"].as_str().unwrap_or("unknown CPU").to_owned();
    progress(&format!("{} on {cpu}", if options.compare { "compare" } else { "df-hyperliquid-core" }));
    if let Some(warning) = machine::power_warning(&machine["power"]) {
        progress(&format!("WARNING: {warning}"));
    }

    let mut result = json!({
        "schema": 1,
        "started_utc": report::now_utc(),
        "options": {
            "compare": options.compare,
            "network": options.network,
            "quick": options.quick,
            "runs": options.runs,
            "profiles": options.profiles,
        },
        "machine": machine,
        "versions": versions,
    });
    let sdks = offline::sdks(&bench, &binaries, options);
    result["signature_check"] = offline::signature_check(&bench, &sdks)?;
    result["copy_check"] = offline::copy_check(&bench, &sdks);
    result["signing"] = offline::signing(&bench, &sdks, options, &mut skipped)?;
    result["parsing"] = offline::parsing(&bench, &sdks, options, &mut skipped)?;
    if options.network {
        result["network"] = network::rounds(&bench, &binaries, options)?;
        result["live"] = network::live(&bench, &binaries, options, &mut skipped)?;
    }
    result["skipped"] = Value::from(skipped);
    result["timing"] = json!({
        "build_s": (build_seconds * 10.0).round() / 10.0,
        "benchmark_s": (measuring.elapsed().as_secs_f64() * 10.0).round() / 10.0,
    });
    report::write(&bench, &result)
}
