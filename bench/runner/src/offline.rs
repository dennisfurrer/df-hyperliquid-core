//! The offline benchmarks, which measure the SDKs rather than the network: the signature check,
//! signing and websocket parsing. Each Rust row runs `runs` times and the run with the median
//! result is reported, with every run's figure kept beside it.

use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value, json};

use crate::compile::Binaries;
use crate::{Options, Result, progress};

pub const DF: &str = "df-hyperliquid-core";
pub const DF_SECP: &str = "df-hyperliquid-core + secp256k1";
pub const HYPERSDK: &str = "hypersdk";
pub const PY: &str = "hyperliquid-python-sdk";
pub const PY_FAST: &str = "hyperliquid-python-sdk + coincurve";

/// The Python SDK's two ECDSA backends: pure Python, its default, and libsecp256k1 through
/// coincurve, which eth-keys uses when coincurve is installed.
const PURE_PYTHON: &str = "eth_keys.backends.NativeECCBackend";
const COINCURVE: &str = "eth_keys.backends.CoinCurveECCBackend";

/// One row: how to start its benchmark program, and whether it parses (the coincurve row signs
/// only; the secp256k1 row parses exactly as the default build does).
pub struct Sdk {
    pub name: &'static str,
    pub program: Vec<String>,
    pub env: Vec<(String, String)>,
    pub parses: bool,
    /// Signing samples for the Python rows, which are far slower.
    pub samples: Option<usize>,
}

impl Sdk {
    fn command(&self, args: &[&str], options: &Options) -> Command {
        let mut command = Command::new(&self.program[0]);
        command.args(&self.program[1..]).args(args);
        command.envs(self.env.iter().map(|(key, value)| (key, value)));
        if options.quick {
            command.env("BENCH_QUICK", "1");
        }
        command
    }
}

fn python(backend: &str, name: &'static str, parses: bool, samples: usize) -> Sdk {
    let program = ["uv", "run", "--project", "py", "--frozen", "python", "py/bench_py.py"];
    Sdk {
        name,
        program: program.iter().map(|part| (*part).to_owned()).collect(),
        env: vec![("ECC_BACKEND_CLASS".to_owned(), backend.to_owned())],
        parses,
        samples: Some(samples),
    }
}

fn rust(name: &'static str, path: &Path, parses: bool) -> Sdk {
    Sdk { name, program: vec![path.display().to_string()], env: Vec::new(), parses, samples: None }
}

pub fn sdks(_bench: &Path, binaries: &Binaries, options: &Options) -> Vec<Sdk> {
    let tenth = |n: usize| if options.quick { n / 10 } else { n };
    let mut rows = vec![rust(DF, &binaries.df, true)];
    if let Some(path) = &binaries.df_secp {
        rows.push(rust(DF_SECP, path, false));
    }
    if let Some(path) = &binaries.hypersdk {
        rows.push(rust(HYPERSDK, path, true));
    }
    if binaries.python {
        rows.push(python(PURE_PYTHON, PY, true, tenth(2_000)));
        rows.push(python(COINCURVE, PY_FAST, false, tenth(5_000)));
    }
    rows
}

/// Runs a benchmark program once and reads the JSON object on its last line of output.
pub fn run_json(bench: &Path, mut command: Command) -> Result<Value> {
    let output = command.current_dir(bench).output().map_err(|error| format!("{command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{command:?} failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let last = stdout.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default();
    serde_json::from_str(last).map_err(|error| format!("{command:?} printed no JSON: {error}"))
}

/// Runs `args` `runs` times and keeps the run whose `metric` is the median, with every run's
/// `metric` under `"runs"`.
fn median_run(bench: &Path, sdk: &Sdk, args: &[&str], options: &Options, metric: fn(&Value) -> f64) -> Result<Value> {
    let mut runs = Vec::with_capacity(options.runs);
    for _ in 0..options.runs {
        runs.push(run_json(bench, sdk.command(args, options))?);
    }
    let figures: Vec<f64> = runs.iter().map(metric).collect();
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by(|a, b| figures[*a].total_cmp(&figures[*b]));
    let mut chosen = runs.swap_remove(order[order.len() / 2]);
    chosen["runs"] = Value::from(figures);
    Ok(chosen)
}

pub fn signature_check(bench: &Path, sdks: &[Sdk]) -> Result<Value> {
    progress("signature check: every SDK signs the same order with the same key and nonce");
    let quiet = Options { compare: false, network: false, quick: false, runs: 1, rounds: 1, live_secs: 5 };
    let mut checks = Vec::new();
    for sdk in sdks {
        let mut check = run_json(bench, sdk.command(&["sign-check"], &quiet))?;
        check["row"] = Value::from(sdk.name);
        checks.push(check);
    }
    let distinct = |key: &str| {
        let mut seen: Vec<String> = checks.iter().map(|check| check[key].to_string()).collect();
        seen.sort();
        seen.dedup();
        seen
    };
    let (signatures, addresses, actions) = (distinct("signature"), distinct("address"), distinct("action"));
    let equal = signatures.len() == 1 && addresses.len() == 1 && actions.len() == 1;
    if !equal {
        return Err(format!("the SDKs signed the fixed order differently; not timing anything: {signatures:?}"));
    }
    let signature = checks[0]["signature"].clone();
    let short: String = signature.as_str().unwrap_or_default().chars().take(18).collect();
    progress(&format!("signature check: all {} rows signed {short}...", checks.len()));
    Ok(json!({ "equal": equal, "signature": signature, "address": checks[0]["address"], "checks": checks }))
}

pub fn signing(bench: &Path, sdks: &[Sdk], options: &Options) -> Result<Value> {
    let mut rows = Map::new();
    for sdk in sdks {
        progress(&format!("signing: {} ({} run{})", sdk.name, options.runs, if options.runs == 1 { "" } else { "s" }));
        let samples = sdk.samples.map(|n| n.to_string());
        let args: Vec<&str> = match &samples {
            Some(n) => vec!["sign", "--samples", n],
            None => vec!["sign"],
        };
        let row = median_run(bench, sdk, &args, options, |run| run["sign"]["median_ns"].as_f64().unwrap_or(f64::MAX))?;
        rows.insert(sdk.name.to_owned(), row);
    }
    Ok(Value::Object(rows))
}

pub fn parsing(bench: &Path, sdks: &[Sdk], options: &Options) -> Result<Value> {
    let corpus = "data/frames.jsonl.gz";
    let mut rows = Map::new();
    for sdk in sdks.iter().filter(|sdk| sdk.parses) {
        progress(&format!("parse: {} over the recorded corpus", sdk.name));
        let row = median_run(bench, sdk, &["parse", "--corpus", corpus], options, |run| {
            run["parse"]["all"]["frames_per_s"].as_f64().unwrap_or(0.0)
        })?;
        rows.insert(sdk.name.to_owned(), row);
    }
    Ok(Value::Object(rows))
}
