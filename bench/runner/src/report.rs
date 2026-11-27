//! What a run leaves behind: `results/<date>-<hhmmss>-<cpu>[-df][-quick]` as `.json` (everything,
//! every raw sample), `.md` (the tables) and `.pdf` (the same, laid out to read and share), and
//! `share`, a summary of at most 280 characters. Paths in the files are relative to the repo; the
//! machine is named by its CPU, never by host or user.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::compile::Binaries;
use crate::offline::{DF, DF_SECP, HYPERSDK, PY, PY_FAST};
use crate::tools::first_line;
use crate::{Result, pdf_report, progress};

/// Days since 1970-01-01 as (year, month, day), proleptic Gregorian.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `2026-11-28T10:15:00Z`.
pub fn now_utc() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64);
    let (year, month, day) = civil(seconds.div_euclid(86_400));
    let time = seconds.rem_euclid(86_400);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", time / 3_600, time % 3_600 / 60, time % 60)
}

fn lock_version(bench: &Path, name: &str) -> Option<String> {
    let lock = std::fs::read_to_string(bench.join("Cargo.lock")).ok()?;
    let block = lock.split("[[package]]").find(|block| block.contains(&format!("name = \"{name}\"\n")))?;
    block.lines().find(|line| line.starts_with("version = ")).and_then(|line| line.split('"').nth(1)).map(str::to_owned)
}

fn workspace_version(bench: &Path) -> Option<String> {
    let manifest = std::fs::read_to_string(bench.join("../Cargo.toml")).ok()?;
    let package = manifest.split("[workspace.package]").nth(1)?;
    package.lines().find(|line| line.starts_with("version")).and_then(|line| line.split('"').nth(1)).map(str::to_owned)
}

pub fn versions(bench: &Path, binaries: &Binaries) -> Value {
    let commit = first_line("git", &["rev-parse", "--short", "HEAD"], bench);
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(bench)
        .output()
        .is_ok_and(|output| !output.stdout.is_empty());
    let mut versions = json!({
        DF: workspace_version(bench),
        "git_commit": commit.map(|commit| if dirty { format!("{commit}-dirty") } else { commit }),
        "rustc": first_line("rustc", &["--version"], bench),
        "features": { DF: "default (k256)", DF_SECP: "secp256k1 (libsecp256k1)" },
    });
    if binaries.hypersdk.is_some() {
        versions[HYPERSDK] = Value::from(lock_version(bench, "hypersdk"));
    }
    if binaries.python {
        let script = "import importlib.metadata as m; print(' '.join(m.version(p) for p in ('hyperliquid-python-sdk', 'eth-account', 'eth-keys', 'coincurve')))";
        let found = first_line("uv", &["run", "--project", "py", "--frozen", "python", "-c", script], bench)
            .unwrap_or_default();
        let found: Vec<&str> = found.split_whitespace().collect();
        for (at, name) in [PY, "eth-account", "eth-keys", "coincurve"].iter().enumerate() {
            versions[*name] = Value::from(found.get(at).map(|version| (*version).to_owned()));
        }
    }
    versions
}

fn us(ns: f64) -> String {
    format!("{:.1}", ns / 1_000.0)
}

fn thousands(value: f64) -> String {
    let digits = format!("{:.0}", value);
    let mut out = String::new();
    for (at, c) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One table: a title, its header and its rows, shared by the Markdown and the PDF. `best` marks
/// the column of the best figure in each row, or `None`.
pub struct Table {
    pub title: String,
    pub header: Vec<String>,
    pub rows: Vec<(Vec<String>, Option<usize>)>,
    pub note: Option<String>,
}

fn f(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

/// The machine, versions and run, as lines, for the top of the Markdown and the PDF.
pub fn heading(result: &Value) -> Vec<String> {
    let machine = &result["machine"];
    let versions = &result["versions"];
    let options = &result["options"];
    let mut competitors = Vec::new();
    for name in [HYPERSDK, PY] {
        if let Some(version) = versions[name].as_str() {
            competitors.push(format!("{name} {version}"));
        }
    }
    let mut lines = vec![
        format!(
            "Machine: {}, {} cores, {} GB, {}, {}",
            machine["cpu"].as_str().unwrap_or("unknown CPU"),
            machine["cores"],
            machine["memory_gb"],
            machine["os"].as_str().unwrap_or_default(),
            machine["arch"].as_str().unwrap_or_default()
        ),
        format!(
            "Versions: {} {} (commit {}){}{}; {}",
            DF,
            versions[DF].as_str().unwrap_or("?"),
            versions["git_commit"].as_str().unwrap_or("unknown"),
            if competitors.is_empty() { "" } else { ", " },
            competitors.join(", "),
            versions["rustc"].as_str().unwrap_or_default()
        ),
        format!(
            "Run: {}, {}, {} run{} per row with the median run reported, started {}",
            if options["compare"].as_bool() == Some(true) { "compare" } else { "df-hyperliquid-core alone" },
            if options["network"].as_bool() == Some(true) { "offline and network" } else { "offline" },
            options["runs"],
            if options["runs"] == 1 { "" } else { "s" },
            result["started_utc"].as_str().unwrap_or_default()
        ),
    ];
    let power = &machine["power"];
    if let (Some(source), mode) = (power["source"].as_str(), power["mode"].as_str()) {
        lines.push(match mode {
            Some(mode) => format!("Power: {source}, {mode} mode"),
            None => format!("Power: {source}"),
        });
    }
    if let Some(warning) = crate::machine::power_warning(power) {
        lines.push(format!("WARNING: {warning}."));
    }
    if options["quick"].as_bool() == Some(true) {
        lines.push("QUICK RUN: fewer samples, to check that the harness runs. Not for publishing.".to_owned());
    }
    lines
}

/// Every table a result has, in order.
pub fn tables(result: &Value) -> Vec<Table> {
    let mut tables = Vec::new();
    let signing = result["signing"].as_object().cloned().unwrap_or_default();
    let fastest = |values: &[f64], lower: bool| -> Option<usize> {
        let mut best = None::<(usize, f64)>;
        for (at, value) in values.iter().enumerate() {
            if *value > 0.0 && best.is_none_or(|(_, b)| if lower { *value < b } else { *value > b }) {
                best = Some((at, *value));
            }
        }
        best.map(|(at, _)| at)
    };
    let sign_best = fastest(&signing.values().map(|row| f(&row["sign"]["median_ns"])).collect::<Vec<_>>(), true);
    let mut sign = Table {
        title: "Signing one BTC limit order (offline)".to_owned(),
        header: ["SDK", "hash + sign median (us)", "p99 (us)", "build + sign + body median (us)", "p99 (us)"]
            .map(str::to_owned)
            .to_vec(),
        rows: Vec::new(),
        note: Some(format!(
            "Same signature from every row: {}.",
            if result["signature_check"]["equal"].as_bool() == Some(true) { "yes" } else { "no" }
        )),
    };
    for (at, (name, row)) in signing.iter().enumerate() {
        let (a, b) = (&row["sign"], &row["build_sign_body"]);
        sign.rows.push((
            vec![
                name.clone(),
                us(f(&a["median_ns"])),
                us(f(&a["p99_ns"])),
                us(f(&b["median_ns"])),
                us(f(&b["p99_ns"])),
            ],
            (sign_best == Some(at)).then_some(1),
        ));
    }
    if let Some(clone) = signing.get(HYPERSDK).and_then(|row| row["clone_action"]["median_ns"].as_f64()) {
        let note = sign.note.take().unwrap_or_default();
        sign.note = Some(format!(
            "{note} hypersdk's sign_sync takes the action by value, so its timed call signs a clone; the clone alone takes {clone:.0} ns at the median."
        ));
    }
    tables.push(sign);

    let recovering: Vec<(&String, &Value)> = signing.iter().filter(|(_, row)| row.get("recover").is_some()).collect();
    if !recovering.is_empty() {
        let best =
            fastest(&recovering.iter().map(|(_, row)| f(&row["recover"]["median_ns"])).collect::<Vec<_>>(), true);
        tables.push(Table {
            title: "Recovering the signer from a signature (offline)".to_owned(),
            header: ["SDK", "median (us)", "p99 (us)"].map(str::to_owned).to_vec(),
            rows: recovering
                .iter()
                .enumerate()
                .map(|(at, (name, row))| {
                    let r = &row["recover"];
                    (vec![(*name).clone(), us(f(&r["median_ns"])), us(f(&r["p99_ns"]))], (best == Some(at)).then_some(1))
                })
                .collect(),
            note: Some("Client::send recovers a wallet's signature before posting; a local key's is not recovered in a release build.".to_owned()),
        });
    }

    let parsing = result["parsing"].as_object().cloned().unwrap_or_default();
    let frames = parsing.values().next().map_or(0.0, |row| f(&row["parse"]["all"]["frames"]));
    let rate = |row: &Value, part: &str| f(&row["parse"][part]["frames_per_s"]);
    let best_rate = fastest(&parsing.values().map(|row| rate(row, "all")).collect::<Vec<_>>(), false);
    tables.push(Table {
        title: format!("Websocket parse ({} recorded mainnet frames, offline)", thousands(frames)),
        header: [
            "SDK",
            "all frames/s",
            "l2Book frames/s",
            "trades frames/s",
            "p99 per frame (ns)",
            "allocations per frame",
            "not read",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows: parsing
            .iter()
            .enumerate()
            .map(|(at, (name, row))| {
                let all = &row["parse"]["all"];
                let allocations = all["allocations_per_frame"].as_f64().map_or("-".to_owned(), |n| format!("{n:.1}"));
                (
                    vec![
                        name.clone(),
                        thousands(rate(row, "all")),
                        thousands(rate(row, "l2Book")),
                        thousands(rate(row, "trades")),
                        thousands(f(&all["frame_p99_ns"])),
                        allocations,
                        all["failed"].to_string(),
                    ],
                    (best_rate == Some(at)).then_some(1),
                )
            })
            .collect(),
        note: parsing.contains_key(PY).then(|| {
            "The Python SDK leaves prices and sizes as strings; the two Rust SDKs read them into exact decimals."
                .to_owned()
        }),
    });

    if let Some(sdks) = result["network"]["sdks"].as_object() {
        for (test, title) in [
            ("info", "Info round trip: POST /info l2Book BTC"),
            ("exchange", "Exchange round trip: refused post-only order from an empty key"),
        ] {
            tables.push(Table {
                title: format!("{title} ({} requests per SDK)", result["network"]["rounds"]),
                header: ["SDK", "median (ms)", "p90 (ms)", "p99 (ms)", "failed"].map(str::to_owned).to_vec(),
                rows: sdks
                    .iter()
                    .map(|(name, row)| {
                        let s = &row[format!("{test}_summary")];
                        let cell = |key: &str| s[key].as_f64().map_or("-".to_owned(), |ms| format!("{ms:.1}"));
                        (
                            vec![
                                name.clone(),
                                cell("median_ms"),
                                cell("p90_ms"),
                                cell("p99_ms"),
                                row[format!("{test}_failed")].to_string(),
                            ],
                            None,
                        )
                    })
                    .collect(),
                note: Some(
                    "Almost all of a round trip is the network and the venue, so these show parity, not a ranking."
                        .to_owned(),
                ),
            });
        }
    }
    if let Some(sdks) = result["live"]["sdks"].as_object() {
        tables.push(Table {
            title: format!(
                "Live websocket, BTC l2Book (fast), bbo and trades, {} s, every SDK at once",
                result["live"]["seconds"]
            ),
            header: ["SDK", "l2Book", "bbo", "trades frames", "not read", "age median (ms)", "age p99 (ms)"]
                .map(str::to_owned)
                .to_vec(),
            rows: sdks
                .iter()
                .map(|(name, row)| {
                    let unread =
                        if row["unreadable"].is_null() { "-".to_owned() } else { row["unreadable"].to_string() };
                    (
                        vec![
                            name.clone(),
                            row["book_frames"].to_string(),
                            row["bbo_frames"].to_string(),
                            row["trade_frames"].to_string(),
                            unread,
                            row["age_ms_median"].to_string(),
                            row["age_ms_p99"].to_string(),
                        ],
                        None,
                    )
                })
                .collect(),
            note: Some(
                "Age is the local clock minus the venue's timestamp; it compares the SDKs with each other.".to_owned(),
            ),
        });
    }
    tables
}

pub fn markdown(result: &Value) -> String {
    let mut out = vec!["# df-hyperliquid-core benchmark".to_owned(), String::new()];
    for line in heading(result) {
        out.push(format!("{line}  "));
    }
    for table in tables(result) {
        out.push(String::new());
        out.push(format!("## {}", table.title));
        out.push(String::new());
        out.push(format!("| {} |", table.header.join(" | ")));
        out.push(format!("|---|{}", "---:|".repeat(table.header.len() - 1)));
        for (cells, best) in &table.rows {
            let cells: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(at, cell)| if Some(at) == *best { format!("**{cell}**") } else { cell.clone() })
                .collect();
            out.push(format!("| {} |", cells.join(" | ")));
        }
        if let Some(note) = &table.note {
            out.push(String::new());
            out.push(note.clone());
        }
    }
    if let Some(skipped) = result["skipped"].as_array().filter(|skipped| !skipped.is_empty()) {
        out.push(String::new());
        out.push("## Skipped".to_owned());
        out.push(String::new());
        out.extend(skipped.iter().map(|note| format!("- {}", note.as_str().unwrap_or_default())));
    }
    out.join("\n") + "\n"
}

fn stem(result: &Value) -> String {
    let started = result["started_utc"].as_str().unwrap_or("unknown");
    let (date, time) = started.split_once('T').unwrap_or((started, ""));
    let time: String = time.chars().filter(char::is_ascii_digit).take(6).collect();
    let mut stem = format!("{date}-{time}-{}", result["machine"]["cpu_slug"].as_str().unwrap_or("unknown"));
    if result["options"]["compare"].as_bool() != Some(true) {
        stem.push_str("-df");
    }
    if result["options"]["quick"].as_bool() == Some(true) {
        stem.push_str("-quick");
    }
    stem
}

pub fn write(bench: &Path, result: &Value) -> Result<()> {
    let folder = bench.join("results");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let stem = stem(result);
    let text = serde_json::to_string_pretty(result).map_err(|error| error.to_string())? + "\n";
    std::fs::write(folder.join(format!("{stem}.json")), text).map_err(|error| error.to_string())?;
    let table = markdown(result);
    std::fs::write(folder.join(format!("{stem}.md")), &table).map_err(|error| error.to_string())?;
    std::fs::write(folder.join(format!("{stem}.pdf")), pdf_report::render(result))
        .map_err(|error| error.to_string())?;
    println!("{table}");
    progress(&format!("wrote bench/results/{stem}.json, .md and .pdf"));
    match summary(result) {
        Ok(line) => println!("Share this ({} characters): {line}", line.chars().count()),
        Err(why) => println!("Not for sharing: {why}"),
    }
    Ok(())
}

/// The latest results file that is not a quick run, a compare run when there is one.
fn latest(bench: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(bench.join("results"))
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .filter(|path| !path.to_string_lossy().ends_with("-quick.json"))
        // `<date>-<hhmmss>-<cpu>`: the runner's own files, not the older single-date ones.
        .filter(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            name.split('-').nth(3).is_some_and(|time| time.len() == 6 && time.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    found.sort();
    // A compare run has ratios to share; a df-hyperliquid-core run alone only its own figures.
    let compare = found.iter().rev().find(|path| !path.to_string_lossy().ends_with("-df.json")).cloned();
    compare.or_else(|| found.pop())
}

fn ratio(ours: f64, theirs: f64) -> Option<f64> {
    (ours > 0.0 && theirs > 0.0).then_some(ours / theirs)
}

/// At most 280 characters: the ratios against each competitor and the machine.
pub fn summary(result: &Value) -> Result<String> {
    if result["options"]["quick"].as_bool() == Some(true) {
        return Err("a quick run is only a check that the harness runs; run `just bench-compare` for numbers to share"
            .to_owned());
    }
    let sign = |name: &str| f(&result["signing"][name]["sign"]["median_ns"]);
    let parse = |name: &str, part: &str| f(&result["parsing"][name]["parse"][part]["frames_per_s"]);
    let machine = format!(
        "{} ({} {})",
        result["machine"]["cpu"].as_str().unwrap_or("unknown CPU"),
        result["machine"]["os"].as_str().unwrap_or_default(),
        result["machine"]["arch"].as_str().unwrap_or_default()
    );
    let version = |name: &str| result["versions"][name].as_str().unwrap_or("?").to_owned();
    let mut parts = Vec::new();
    if result["parsing"].get(HYPERSDK).is_some() {
        let mut line = format!("vs hypersdk {}:", version(HYPERSDK));
        if let (Some(all), Some(books)) =
            (ratio(parse(DF, "all"), parse(HYPERSDK, "all")), ratio(parse(DF, "l2Book"), parse(HYPERSDK, "l2Book")))
        {
            line.push_str(&format!(" parse {all:.1}x (books {books:.1}x),"));
        }
        if let Some(signing) = ratio(sign(HYPERSDK), sign(DF)) {
            line.push_str(&format!(" sign {signing:.1}x"));
            if let Some(fast) = ratio(sign(HYPERSDK), sign(DF_SECP)) {
                line.push_str(&format!(" ({fast:.1}x with secp256k1)"));
            }
        }
        parts.push(line.trim_end_matches(',').to_owned());
    }
    if result["parsing"].get(PY).is_some() {
        let mut line = format!("vs Python SDK {}:", version(PY));
        if let Some(all) = ratio(parse(DF, "all"), parse(PY, "all")) {
            line.push_str(&format!(" parse {all:.1}x,"));
        }
        if let Some(signing) = ratio(sign(PY_FAST), sign(DF)) {
            line.push_str(&format!(" sign {signing:.1}x (coincurve)"));
        }
        parts.push(line.trim_end_matches(',').to_owned());
    }
    let head = format!("df-hyperliquid-core {} on {machine}", version(DF));
    let mut text = if parts.is_empty() {
        format!(
            "{head}, df-hyperliquid-core only, no comparison: sign {} us, parse {} frames/s",
            us(sign(DF)),
            thousands(parse(DF, "all"))
        )
    } else {
        if result["parsing"].get(HYPERSDK).is_none() {
            // First, so it stays when the Python part is dropped below.
            parts.insert(0, "no hypersdk comparison".to_owned());
        }
        format!("{head}, {}", parts.join("; "))
    };
    text.push_str(". github.com/dennisfurrer/df-hyperliquid-core/tree/main/bench");
    if text.chars().count() > 280 {
        // Drop the Python part first, then the link, to stay within one post.
        text = format!(
            "{head}, {}. github.com/dennisfurrer/df-hyperliquid-core",
            parts.first().cloned().unwrap_or_default()
        );
    }
    if text.chars().count() > 280 {
        text = text.chars().take(280).collect();
    }
    Ok(text)
}

pub fn share(bench: &Path, given: Option<PathBuf>) -> Result<()> {
    let path = given.or_else(|| latest(bench)).ok_or("no results yet; run `just bench-compare` first")?;
    let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let result: Value = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let line = summary(&result)?;
    println!("{line}");
    eprintln!(
        "({} characters, from {})",
        line.chars().count(),
        path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    );
    if let Some(warning) = crate::machine::power_warning(&result["machine"]["power"]) {
        eprintln!("Note: this run was {warning}.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::summary;
    use crate::offline::{DF, HYPERSDK, PY, PY_FAST};

    fn result(rows: &[&str]) -> Value {
        let mut result = json!({
            "options": { "quick": false },
            "machine": { "cpu": "Apple M5 Max", "os": "macOS 26.5", "arch": "aarch64" },
            "versions": { DF: "0.2.5", HYPERSDK: "0.1.0", PY: "0.20.0" },
            "signing": {},
            "parsing": {},
        });
        for name in rows {
            result["signing"][*name] = json!({ "sign": { "median_ns": 30_000.0 } });
            result["parsing"][*name] =
                json!({ "parse": { "all": { "frames_per_s": 500_000.0 }, "l2Book": { "frames_per_s": 300_000.0 } } });
        }
        result["signing"][PY_FAST] = json!({ "sign": { "median_ns": 90_000.0 } });
        result
    }

    #[test]
    fn a_run_without_competitors_says_it_is_no_comparison() {
        let text = summary(&result(&[DF])).unwrap();
        assert!(text.contains("df-hyperliquid-core only, no comparison"), "{text}");
    }

    #[test]
    fn a_run_without_hypersdk_says_so() {
        let text = summary(&result(&[DF, PY])).unwrap();
        assert!(text.contains("no hypersdk comparison"), "{text}");
        assert!(text.contains("vs Python SDK"), "{text}");
    }

    #[test]
    fn a_compare_run_gives_ratios() {
        let text = summary(&result(&[DF, HYPERSDK, PY])).unwrap();
        assert!(text.contains("vs hypersdk 0.1.0"), "{text}");
        assert!(!text.contains("no comparison") && !text.contains("no hypersdk"), "{text}");
        assert!(text.chars().count() <= 280);
    }
}
