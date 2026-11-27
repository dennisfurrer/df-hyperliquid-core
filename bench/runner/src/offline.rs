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
pub const LHERMOSO: &str = "hyperliquid-sdk-rs";
pub const QUICKNODE: &str = "quicknode-hyperliquid-sdk";
pub const OFFICIAL_RUST: &str = "hyperliquid_rust_sdk";
pub const NKTKAS: &str = "@nktkas/hyperliquid";
pub const GO: &str = "go-hyperliquid";
pub const PY: &str = "hyperliquid-python-sdk";
pub const PY_FAST: &str = "hyperliquid-python-sdk + coincurve";

/// The Python SDK's two ECDSA backends: pure Python, its default, and libsecp256k1 through
/// coincurve, which eth-keys uses when coincurve is installed.
const PURE_PYTHON: &str = "eth_keys.backends.NativeECCBackend";
const COINCURVE: &str = "eth_keys.backends.CoinCurveECCBackend";

/// Every competitor row, in the order the tables list them.
pub const COMPETITORS: [&str; 8] = [HYPERSDK, LHERMOSO, QUICKNODE, OFFICIAL_RUST, NKTKAS, GO, PY, PY_FAST];

/// One row: how to start its benchmark program, and whether it parses (the coincurve row signs
/// only; the secp256k1 row parses exactly as the default build does).
pub struct Sdk {
    pub name: &'static str,
    pub program: Vec<String>,
    pub env: Vec<(String, String)>,
    pub parses: bool,
    /// Signing samples for the Python rows, which are far slower.
    pub samples: Option<usize>,
    /// The build profile of a Rust row, `None` for the others.
    pub profile: Option<&'static str>,
    /// A Rust row built with the second profile of `--profile both`: its figures go under
    /// `"profiles"` in the row of the same name.
    pub extra: bool,
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
        profile: None,
        extra: false,
    }
}

fn rust(name: &'static str, path: &Path, parses: bool, profile: &'static str, extra: bool) -> Sdk {
    Sdk {
        name,
        program: vec![path.display().to_string()],
        env: Vec::new(),
        parses,
        samples: None,
        profile: Some(profile),
        extra,
    }
}

/// The Rust rows of one profile's binaries.
fn rust_rows(binaries: &Binaries, extra: bool) -> Vec<Sdk> {
    let profile = binaries.profile;
    let mut rows = vec![rust(DF, &binaries.df, true, profile, extra)];
    if let Some(path) = &binaries.df_secp {
        rows.push(rust(DF_SECP, path, false, profile, extra));
    }
    if let Some(path) = &binaries.hypersdk {
        rows.push(rust(HYPERSDK, path, true, profile, extra));
    }
    for (name, path) in
        [(LHERMOSO, &binaries.lhermoso), (QUICKNODE, &binaries.quicknode), (OFFICIAL_RUST, &binaries.official_rust)]
    {
        if let Some(path) = path {
            rows.push(rust(name, path, true, profile, extra));
        }
    }
    rows
}

pub fn sdks(_bench: &Path, binaries: &Binaries, options: &Options) -> Vec<Sdk> {
    let tenth = |n: usize| if options.quick { n / 10 } else { n };
    let mut rows = rust_rows(binaries, false);
    if binaries.ts {
        let program = ["node", "ts/bench.mjs"];
        rows.push(Sdk {
            name: NKTKAS,
            program: program.iter().map(|part| (*part).to_owned()).collect(),
            env: Vec::new(),
            parses: true,
            samples: None,
            profile: None,
            extra: false,
        });
    }
    if let Some(path) = &binaries.go {
        rows.push(Sdk {
            name: GO,
            program: vec![path.display().to_string()],
            env: Vec::new(),
            parses: true,
            samples: None,
            profile: None,
            extra: false,
        });
    }
    if binaries.python {
        rows.push(python(PURE_PYTHON, PY, true, tenth(2_000)));
        rows.push(python(COINCURVE, PY_FAST, false, tenth(5_000)));
    }
    if let Some(other) = &binaries.other {
        rows.extend(rust_rows(other, true));
    }
    rows
}

/// A row's name for progress lines and notes: with its profile when it is the second one.
fn label(sdk: &Sdk) -> String {
    match (sdk.extra, sdk.profile) {
        (true, Some(profile)) => format!("{} ({profile})", sdk.name),
        _ => sdk.name.to_owned(),
    }
}

/// Keeps a measured row: a first-profile row under its name with its profile, a second-profile
/// row under `"profiles"` in the first-profile row of the same name.
fn keep(rows: &mut Map<String, Value>, sdk: &Sdk, mut row: Value) {
    if let Some(profile) = sdk.profile {
        row["profile"] = Value::from(profile);
    }
    match (sdk.extra, sdk.profile) {
        (true, Some(profile)) => {
            if let Some(first) = rows.get_mut(sdk.name) {
                first["profiles"][profile] = row;
            }
        }
        _ => {
            rows.insert(sdk.name.to_owned(), row);
        }
    }
}

/// Options for a single unmeasured call (`sign-check`, `verify-copy`).
fn quiet() -> Options {
    Options { compare: false, network: false, quick: false, runs: 1, rounds: 1, live_secs: 5, profiles: Vec::new() }
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

/// `value` as compact JSON with every object's keys sorted, so two SDKs that write the same
/// action with its keys in another order compare equal.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let fields: Vec<String> =
                keys.iter().map(|key| format!("{}:{}", Value::from(key.as_str()), canonical(&map[*key]))).collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

/// Every row signs the same order with the same key and nonce. A row whose program fails, or
/// whose signature, address or action differs from df-hyperliquid-core's, is listed under
/// `"failing"` with the reason; it is still timed, and the tables say it failed the check.
pub fn signature_check(bench: &Path, sdks: &[Sdk]) -> Result<Value> {
    progress("signature check: every SDK signs the same order with the same key and nonce");
    let quiet = quiet();
    let mut checks = Vec::new();
    let mut failing = Vec::new();
    for sdk in sdks.iter().filter(|sdk| !sdk.extra) {
        match run_json(bench, sdk.command(&["sign-check"], &quiet)) {
            Ok(mut check) => {
                check["row"] = Value::from(sdk.name);
                checks.push(check);
            }
            Err(error) => failing.push(json!({ "row": sdk.name, "reason": first_line(&error) })),
        }
    }
    let reference = checks
        .iter()
        .find(|check| check["row"] == DF)
        .cloned()
        .ok_or("df-hyperliquid-core's sign-check did not run")?;
    for check in &checks {
        let differs: Vec<&str> = ["signature", "address", "action"]
            .into_iter()
            .filter(|key| canonical(&check[*key]) != canonical(&reference[*key]))
            .collect();
        if !differs.is_empty() {
            failing.push(json!({
                "row": check["row"],
                "reason": format!("its {} differs from df-hyperliquid-core's", differs.join(", ")),
            }));
        }
    }
    let signature = reference["signature"].clone();
    let short: String = signature.as_str().unwrap_or_default().chars().take(18).collect();
    if failing.is_empty() {
        progress(&format!("signature check: all {} rows signed {short}...", checks.len()));
    } else {
        for failed in &failing {
            progress(&format!(
                "signature check FAILED for {}: {}",
                failed["row"].as_str().unwrap_or_default(),
                failed["reason"].as_str().unwrap_or_default()
            ));
        }
    }
    Ok(json!({
        "equal": failing.is_empty(),
        "signature": signature,
        "address": reference["address"],
        "checks": checks,
        "failing": failing,
    }))
}

/// hyperliquid-sdk-rs and hyperliquid_rust_sdk sign through private code, which their harnesses
/// copy. `verify-copy` runs each SDK's own public order call without reaching the venue and
/// compares what it signs or posts with what the copy makes from the same order and nonce.
pub fn copy_check(bench: &Path, sdks: &[Sdk]) -> Value {
    let quiet = quiet();
    let mut rows = Map::new();
    for sdk in sdks.iter().filter(|sdk| !sdk.extra && [LHERMOSO, OFFICIAL_RUST].contains(&sdk.name)) {
        progress(&format!("copy check: {}'s copied signing code against its own order call", sdk.name));
        let row = match run_json(bench, sdk.command(&["verify-copy"], &quiet)) {
            Ok(answer) => answer["verify_copy"].clone(),
            Err(error) => json!({ "equal": false, "reason": first_line(&error) }),
        };
        if row["equal"].as_bool() != Some(true) {
            progress(&format!("copy check FAILED for {}", sdk.name));
        }
        rows.insert(sdk.name.to_owned(), row);
    }
    Value::Object(rows)
}

/// The first line of an error, for a note.
pub fn first_line(error: &str) -> String {
    error.lines().find(|line| !line.trim().is_empty()).unwrap_or_default().chars().take(200).collect()
}

/// Runs `args` for one row: df-hyperliquid-core's failure stops the run, a competitor's is noted
/// in `skipped` and its row left out.
fn measure(
    bench: &Path,
    sdk: &Sdk,
    args: &[&str],
    options: &Options,
    metric: fn(&Value) -> f64,
    skipped: &mut Vec<String>,
) -> Result<Option<Value>> {
    match median_run(bench, sdk, args, options, metric) {
        Ok(row) => Ok(Some(row)),
        Err(error) if sdk.name != DF || sdk.extra => {
            skipped.push(format!("{}: `{}` failed: {}", label(sdk), args[0], first_line(&error)));
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

pub fn signing(bench: &Path, sdks: &[Sdk], options: &Options, skipped: &mut Vec<String>) -> Result<Value> {
    let mut rows = Map::new();
    for sdk in sdks {
        progress(&format!(
            "signing: {} ({} run{})",
            label(sdk),
            options.runs,
            if options.runs == 1 { "" } else { "s" }
        ));
        let samples = sdk.samples.map(|n| n.to_string());
        let args: Vec<&str> = match &samples {
            Some(n) => vec!["sign", "--samples", n],
            None => vec!["sign"],
        };
        let metric: fn(&Value) -> f64 = |run| run["sign"]["median_ns"].as_f64().unwrap_or(f64::MAX);
        if let Some(row) = measure(bench, sdk, &args, options, metric, skipped)? {
            keep(&mut rows, sdk, row);
        }
    }
    Ok(Value::Object(rows))
}

pub fn parsing(bench: &Path, sdks: &[Sdk], options: &Options, skipped: &mut Vec<String>) -> Result<Value> {
    let corpus = "data/frames.jsonl.gz";
    let mut rows = Map::new();
    for sdk in sdks.iter().filter(|sdk| sdk.parses) {
        progress(&format!("parse: {} over the recorded corpus", label(sdk)));
        let metric: fn(&Value) -> f64 = |run| run["parse"]["all"]["frames_per_s"].as_f64().unwrap_or(0.0);
        if let Some(row) = measure(bench, sdk, &["parse", "--corpus", corpus], options, metric, skipped)? {
            keep(&mut rows, sdk, row);
        }
    }
    Ok(Value::Object(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_action_with_its_keys_in_another_order_is_the_same() {
        let a: Value = serde_json::from_str(r#"{"type":"order","orders":[{"a":0,"b":true}],"grouping":"na"}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"grouping":"na","orders":[{"b":true,"a":0}],"type":"order"}"#).unwrap();
        let c: Value =
            serde_json::from_str(r#"{"grouping":"na","orders":[{"b":false,"a":0}],"type":"order"}"#).unwrap();
        assert_eq!(canonical(&a), canonical(&b));
        assert_ne!(canonical(&a), canonical(&c));
    }
}
