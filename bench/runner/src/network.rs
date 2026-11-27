//! The opt-in network benchmarks (`--network`): info and exchange round trips on mainnet, and
//! the live websocket. Only this module reaches the venue. The exchange round trip posts a
//! post-only BTC buy 30% under the mid, signed by a key each SDK generates at start: the key has
//! never held anything, so the venue has no account for it and refuses the order. Nothing can
//! fill, and no key is ever read from the user.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::compile::Binaries;
use crate::offline::{DF, HYPERSDK, PY, run_json};
use crate::{Options, Result, progress};

const WARMUP_ROUNDS: usize = 3;
const GAP: Duration = Duration::from_millis(300);

/// The commands that start each SDK's program, with `args` after them.
fn programs(binaries: &Binaries, args: &[&str]) -> Vec<(&'static str, Command)> {
    let rust = |path: &Path| {
        let mut command = Command::new(path);
        command.args(args);
        command
    };
    let mut programs = vec![(DF, rust(&binaries.df))];
    if let Some(path) = &binaries.hypersdk {
        programs.push((HYPERSDK, rust(path)));
    }
    if binaries.python {
        let mut command = Command::new("uv");
        command.args(["run", "--project", "py", "--frozen", "python", "py/bench_py.py"]).args(args);
        command.env("ECC_BACKEND_CLASS", "eth_keys.backends.NativeECCBackend");
        programs.push((PY, command));
    }
    programs
}

/// A post-only buy 30% under the BTC mid on whole dollars, and a size worth about $12.
fn order(bench: &Path, binaries: &Binaries) -> Result<(String, String)> {
    let mut command = Command::new(&binaries.df);
    command.arg("mid");
    let mid: f64 = run_json(bench, command)?["mid"].as_str().and_then(|mid| mid.parse().ok()).ok_or("no BTC mid")?;
    let px = format!("{:.5e}", mid * 0.7).parse::<f64>().map_err(|error| error.to_string())?.floor();
    let sz = (12.0 / px * 1e5).ceil() / 1e5;
    Ok((format!("{px:.0}"), format!("{sz:.5}")))
}

/// One SDK's long-lived process: a line in, one JSON line out.
struct Worker {
    name: &'static str,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Worker {
    fn start(bench: &Path, name: &'static str, mut command: Command) -> Result<Worker> {
        let mut child = command
            .current_dir(bench)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("{name}: {error}"))?;
        let input = child.stdin.take().ok_or("no stdin")?;
        let output = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        Ok(Worker { name, child, input, output })
    }

    fn ask(&mut self, line: &str) -> Result<Value> {
        writeln!(self.input, "{line}")
            .and_then(|()| self.input.flush())
            .map_err(|error| format!("{}: {error}", self.name))?;
        let mut answer = String::new();
        self.output.read_line(&mut answer).map_err(|error| format!("{}: {error}", self.name))?;
        serde_json::from_str(&answer).map_err(|_| format!("{} worker stopped", self.name))
    }

    fn stop(mut self) {
        let _ = writeln!(self.input, "quit");
        let _ = self.child.wait();
    }
}

fn summary_ms(samples: &[u64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        return json!({});
    }
    let at = |fraction: f64| sorted[((sorted.len() - 1) as f64 * fraction).round() as usize] as f64 / 1e6;
    json!({
        "n": sorted.len(),
        "min_ms": sorted[0] as f64 / 1e6,
        "median_ms": at(0.5),
        "p90_ms": at(0.9),
        "p99_ms": at(0.99),
        "max_ms": sorted[sorted.len() - 1] as f64 / 1e6,
    })
}

pub fn rounds(bench: &Path, binaries: &Binaries, options: &Options) -> Result<Value> {
    progress("network: these results depend mostly on the distance to the venue, not on the SDK");
    let (px, sz) = order(bench, binaries)?;
    progress(&format!("network: {} rounds, a post-only BTC buy of {sz} at {px} from a new empty key", options.rounds));
    let mut workers = Vec::new();
    for (name, command) in programs(binaries, &["serve", "--px", &px, "--sz", &sz]) {
        workers.push(Worker::start(bench, name, command)?);
    }
    let mut samples: Vec<[Vec<(bool, u64, String)>; 2]> = workers.iter().map(|_| [Vec::new(), Vec::new()]).collect();
    for round in 0..WARMUP_ROUNDS + options.rounds {
        // Rotate who goes first, so no SDK always follows the same neighbour.
        let order: Vec<usize> = (0..workers.len()).map(|at| (at + round) % workers.len()).collect();
        for (test, name) in ["info", "exchange"].iter().enumerate() {
            for &at in &order {
                let answer = workers[at].ask(name)?;
                if round >= WARMUP_ROUNDS {
                    let ok = answer["ok"].as_bool().unwrap_or(false);
                    let ns = answer["ns"].as_u64().unwrap_or(0);
                    samples[at][test].push((ok, ns, answer["answer"].as_str().unwrap_or_default().to_owned()));
                }
                std::thread::sleep(GAP);
            }
        }
        if (round + 1) % 10 == 0 {
            progress(&format!("network: round {}/{}", round + 1, WARMUP_ROUNDS + options.rounds));
        }
    }
    let mut rows = Map::new();
    for (worker, tests) in workers.into_iter().zip(samples) {
        let mut row = Map::new();
        for (test, answers) in ["info", "exchange"].iter().zip(tests) {
            let ok: Vec<u64> = answers.iter().filter(|(ok, _, _)| *ok).map(|(_, ns, _)| *ns).collect();
            row.insert(format!("{test}_summary"), summary_ms(&ok));
            row.insert(format!("{test}_failed"), Value::from(answers.len() - ok.len()));
            row.insert(format!("{test}_first_answer"), Value::from(answers.first().map(|(_, _, text)| text.clone())));
            row.insert(
                format!("{test}_samples_ns"),
                Value::from(answers.iter().map(|(_, ns, _)| *ns).collect::<Vec<_>>()),
            );
        }
        rows.insert(worker.name.to_owned(), Value::Object(row));
        worker.stop();
    }
    Ok(
        json!({ "order": { "coin": "BTC", "side": "buy", "tif": "Alo", "px": px, "sz": sz }, "rounds": options.rounds, "sdks": rows }),
    )
}

pub fn live(bench: &Path, binaries: &Binaries, options: &Options) -> Result<Value> {
    let secs = options.live_secs.to_string();
    progress(&format!("live: BTC l2Book (fast), bbo and trades, every SDK at once for {secs} s"));
    let running: Vec<(&str, std::thread::JoinHandle<Result<Value>>)> = programs(binaries, &["live", "--secs", &secs])
        .into_iter()
        .map(|(name, mut command)| {
            let bench = bench.to_path_buf();
            (
                name,
                std::thread::spawn(move || {
                    command.current_dir(&bench).output().map_err(|e| e.to_string()).and_then(|out| {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let last = text.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default();
                        serde_json::from_str::<Value>(last)
                            .map(|value| value["live"].clone())
                            .map_err(|e| format!("live: {e}"))
                    })
                }),
            )
        })
        .collect();
    let mut rows = Map::new();
    for (name, handle) in running {
        let row = handle.join().map_err(|_| format!("{name}: live run panicked"))??;
        rows.insert(name.to_owned(), row);
    }
    Ok(json!({ "seconds": options.live_secs, "sdks": rows }))
}
