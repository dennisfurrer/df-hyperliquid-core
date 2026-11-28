//! What a run leaves behind: `results/<date>-<hhmmss>-<cpu>[-df][-quick]` as `.json` (everything,
//! every raw sample), `.md` (the tables) and `.pdf` (the same, laid out to read and share), and
//! `share`, a summary of at most 280 characters. Paths in the files are relative to the repo; the
//! machine is named by its CPU, never by host or user.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::compile::Binaries;
use crate::offline::{COMPETITORS, DF, DF_SECP, GO, HYPERSDK, LHERMOSO, NKTKAS, OFFICIAL_RUST, PY, PY_FAST, QUICKNODE};
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
    lock_version_in(&bench.join("Cargo.lock"), name)
}

/// The version of package `name` in the Cargo.lock at `lock`.
fn lock_version_in(lock: &Path, name: &str) -> Option<String> {
    let lock = std::fs::read_to_string(lock).ok()?;
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
    if binaries.lhermoso.is_some() {
        versions[LHERMOSO] = Value::from(lock_version_in(&bench.join("lhermoso/Cargo.lock"), "hyperliquid-sdk-rs"));
    }
    if binaries.quicknode.is_some() {
        versions[QUICKNODE] =
            Value::from(lock_version_in(&bench.join("quicknode/Cargo.lock"), "quicknode-hyperliquid-sdk"));
    }
    if binaries.official_rust.is_some() {
        versions[OFFICIAL_RUST] =
            Value::from(lock_version_in(&bench.join("official-rust/Cargo.lock"), "hyperliquid_rust_sdk"));
    }
    if binaries.ts {
        let package = std::fs::read_to_string(bench.join("ts/node_modules/@nktkas/hyperliquid/package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        versions[NKTKAS] = package.map_or(Value::Null, |package| package["version"].clone());
        versions["node"] = Value::from(first_line("node", &["--version"], bench));
    }
    if binaries.go.is_some() {
        let module = std::fs::read_to_string(bench.join("go/go.mod")).ok().and_then(|text| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix("github.com/sonirico/go-hyperliquid v").map(str::to_owned))
        });
        versions[GO] = Value::from(module);
        versions["go"] = Value::from(first_line("go", &["version"], bench));
        // Whether cgo was on decides go-ethereum's secp256k1: libsecp256k1 (C) or a Go library.
        versions["go_build"] = binaries
            .go
            .as_ref()
            .and_then(|path| Command::new(path).arg("build-info").output().ok())
            .filter(|output| output.status.success())
            .and_then(|output| serde_json::from_slice::<Value>(&output.stdout).ok())
            .unwrap_or(Value::Null);
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
    for name in COMPETITORS {
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
    let profiles: Vec<&str> = options["profiles"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    match profiles.as_slice() {
        [] => {}
        [one] => lines.push(format!("Rust build profile: {}", profile_text(one))),
        [first, second, ..] => lines.push(format!(
            "Rust build profiles: {} in the main columns, {} in the columns marked {second}",
            profile_text(first),
            profile_text(second)
        )),
    }
    let go_build = &versions["go_build"];
    if let Some(secp256k1) = go_build["secp256k1"].as_str() {
        lines.push(format!(
            "Go build: cgo {}; go-ethereum signs with {secp256k1}",
            if go_build["cgo"].as_bool() == Some(true) { "on" } else { "off" }
        ));
    }
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

/// A Rust build profile in words.
fn profile_text(profile: &str) -> String {
    match profile {
        "release" => "release (cargo's default release profile)".to_owned(),
        "release-lto" => "release-lto (fat link-time optimization, one codegen unit)".to_owned(),
        other => other.to_owned(),
    }
}

/// A figure in microseconds, or `-` when there is none.
fn us_or_dash(ns: &Value) -> String {
    ns.as_f64().map_or("-".to_owned(), us)
}

/// The live table's note: how age is measured, and the rows that cannot subscribe the same way.
fn live_note(sdks: &serde_json::Map<String, Value>) -> String {
    let mut note =
        "Age is the local clock minus the venue's timestamp; it compares the SDKs with each other.".to_owned();
    let applies = |name: &str| sdks.get(name).is_some_and(|row| row["not_applicable"].is_null());
    let slow: Vec<&str> = [LHERMOSO, QUICKNODE, OFFICIAL_RUST].into_iter().filter(|name| applies(name)).collect();
    if !slow.is_empty() {
        note.push_str(&format!(
            " {} have no fast l2Book option, so they get the normal book, sent less often.",
            slow.join(", ")
        ));
    }
    if sdks.contains_key(OFFICIAL_RUST) {
        note.push_str(&format!(" {OFFICIAL_RUST} has no bbo subscription."));
    }
    if let Some(reason) = sdks.get(QUICKNODE).and_then(|row| row["not_applicable"].as_str()) {
        note.push_str(&format!(" {QUICKNODE}: not applicable, {reason}; it is left out of the age columns."));
    }
    if let Some(error) = sdks.get(QUICKNODE).and_then(|row| row["venue_error"].as_str()) {
        let error: String = error.chars().take(60).collect();
        note.push_str(&format!(
            " {QUICKNODE}'s Stream documents a QuickNode endpoint; without one it sends its subscriptions to the public websocket, which refuses them (\"{error}...\"), so it receives no market data."
        ));
    }
    note
}

/// The round-trip tables' note: they show parity, and a row through another service is not
/// compared with the rest.
fn round_trip_note(sdks: &serde_json::Map<String, Value>) -> String {
    let mut note =
        "Almost all of a round trip is the network and the venue, so these show parity, not a ranking.".to_owned();
    if let Some(route) = sdks.get(QUICKNODE).and_then(|row| row["route"].as_str()) {
        note.push_str(&format!(
            " {QUICKNODE}, via QuickNode: its requests go through {route}, not to the venue's API directly, so its row measures a different path and is not ranked with the others."
        ));
    }
    note
}

/// What each parsing row reads a frame into, for the rows present.
fn parse_note(parsing: &serde_json::Map<String, Value>) -> String {
    let reads = [
        (DF, "typed values with exact decimals"),
        (HYPERSDK, "typed values with exact decimals"),
        (LHERMOSO, "typed values with strings, with simd-json"),
        (QUICKNODE, "an untyped serde_json::Value"),
        (OFFICIAL_RUST, "typed values with strings"),
        (NKTKAS, "JSON.parse objects, dispatched as events"),
        (GO, "typed values with float64 book levels"),
        (PY, "dicts of strings"),
    ];
    let parts: Vec<String> = reads
        .iter()
        .filter(|(name, _)| parsing.contains_key(*name))
        .map(|(name, what)| format!("{name}: {what}"))
        .collect();
    format!(
        "What each row reads a frame into: {}. Allocations are counted where the runtime can count them.",
        parts.join("; ")
    )
}

/// Whether every row signed the same order the same way, and which did not and why.
fn signature_note(check: &Value, copies: &Value) -> String {
    let mut note = format!(
        "Same signature from every row: {}.",
        if check["equal"].as_bool() == Some(true) { "yes" } else { "no" }
    );
    for failed in check["failing"].as_array().into_iter().flatten() {
        note.push_str(&format!(
            " FAILED: {}, {}.",
            failed["row"].as_str().unwrap_or_default(),
            failed["reason"].as_str().unwrap_or_default()
        ));
    }
    for (name, copy) in copies.as_object().into_iter().flatten() {
        if copy["equal"].as_bool() == Some(true) {
            note.push_str(&format!(
                " {name} signs through private code, which its harness copies; the copy made the same bytes as the SDK's own order call."
            ));
        } else {
            note.push_str(&format!(
                " FAILED: {name}'s copied signing code did not make the same bytes as the SDK's own order call{}.",
                copy["reason"].as_str().map(|reason| format!(" ({reason})")).unwrap_or_default()
            ));
        }
    }
    note
}

/// A figure, or `-` when the row has none.
fn cell(value: &Value) -> String {
    if value.is_null() { "-".to_owned() } else { value.to_string() }
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
    // The second build profile of `--profile both`, whose figures are extra columns.
    let second = result["options"]["profiles"][1].as_str().map(str::to_owned);
    let mut sign_header: Vec<String> =
        ["SDK", "hash + sign median (us)", "p99 (us)", "build + sign + body median (us)", "p99 (us)"]
            .map(str::to_owned)
            .to_vec();
    if let Some(second) = &second {
        sign_header.push(format!("hash + sign median, {second} (us)"));
        sign_header.push(format!("build + sign + body median, {second} (us)"));
    }
    let mut sign = Table {
        title: "Signing one BTC limit order (offline)".to_owned(),
        header: sign_header,
        rows: Vec::new(),
        note: Some(signature_note(&result["signature_check"], &result["copy_check"])),
    };
    for (at, (name, row)) in signing.iter().enumerate() {
        let (a, b) = (&row["sign"], &row["build_sign_body"]);
        let mut cells = vec![
            name.clone(),
            us(f(&a["median_ns"])),
            us(f(&a["p99_ns"])),
            us(f(&b["median_ns"])),
            us(f(&b["p99_ns"])),
        ];
        if let Some(second) = &second {
            let other = &row["profiles"][second.as_str()];
            cells.push(us_or_dash(&other["sign"]["median_ns"]));
            cells.push(us_or_dash(&other["build_sign_body"]["median_ns"]));
        }
        sign.rows.push((cells, (sign_best == Some(at)).then_some(1)));
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
            header: {
                let mut header = ["SDK", "median (us)", "p99 (us)"].map(str::to_owned).to_vec();
                if let Some(second) = &second {
                    header.push(format!("median, {second} (us)"));
                }
                header
            },
            rows: recovering
                .iter()
                .enumerate()
                .map(|(at, (name, row))| {
                    let r = &row["recover"];
                    let mut cells = vec![(*name).clone(), us(f(&r["median_ns"])), us(f(&r["p99_ns"]))];
                    if let Some(second) = &second {
                        cells.push(us_or_dash(&row["profiles"][second.as_str()]["recover"]["median_ns"]));
                    }
                    (cells, (best == Some(at)).then_some(1))
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
        header: {
            let mut header = [
                "SDK",
                "all frames/s",
                "l2Book frames/s",
                "trades frames/s",
                "p99 per frame (ns)",
                "allocations per frame",
                "not read",
            ]
            .map(str::to_owned)
            .to_vec();
            if let Some(second) = &second {
                header.push(format!("all frames/s, {second}"));
            }
            header
        },
        rows: parsing
            .iter()
            .enumerate()
            .map(|(at, (name, row))| {
                let all = &row["parse"]["all"];
                let allocations = all["allocations_per_frame"].as_f64().map_or("-".to_owned(), |n| format!("{n:.1}"));
                let mut cells = vec![
                    name.clone(),
                    thousands(rate(row, "all")),
                    thousands(rate(row, "l2Book")),
                    thousands(rate(row, "trades")),
                    thousands(f(&all["frame_p99_ns"])),
                    allocations,
                    all["failed"].to_string(),
                ];
                if let Some(second) = &second {
                    let other = &row["profiles"][second.as_str()]["parse"]["all"]["frames_per_s"];
                    cells.push(other.as_f64().map_or("-".to_owned(), thousands));
                }
                (cells, (best_rate == Some(at)).then_some(1))
            })
            .collect(),
        note: (parsing.len() > 1).then(|| parse_note(&parsing)),
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
                        let name = match row["route"].as_str() {
                            Some(_) => format!("{name}, via QuickNode"),
                            None => name.clone(),
                        };
                        let s = &row[format!("{test}_summary")];
                        let cell = |key: &str| s[key].as_f64().map_or("-".to_owned(), |ms| format!("{ms:.1}"));
                        (
                            vec![
                                name,
                                cell("median_ms"),
                                cell("p90_ms"),
                                cell("p99_ms"),
                                row[format!("{test}_failed")].to_string(),
                            ],
                            None,
                        )
                    })
                    .collect(),
                note: Some(round_trip_note(sdks)),
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
                    if !row["not_applicable"].is_null() {
                        let mut cells = vec![name.clone()];
                        cells.extend(std::iter::repeat_n("n/a".to_owned(), 6));
                        return (cells, None);
                    }
                    (
                        vec![
                            name.clone(),
                            cell(&row["book_frames"]),
                            cell(&row["bbo_frames"]),
                            cell(&row["trade_frames"]),
                            cell(&row["unreadable"]),
                            cell(&row["age_ms_median"]),
                            cell(&row["age_ms_p99"]),
                        ],
                        None,
                    )
                })
                .collect(),
            note: Some(live_note(sdks)),
        });
    }
    tables
}

/// Every figure in one table: a row for each SDK and each signing or parsing variant of it, and with
/// `--profile both` a second row for the second build profile. Numbers only; `-` where a test does
/// not apply to that row.
pub fn summary_table(result: &Value) -> Table {
    let empty = serde_json::Map::new();
    let signing = result["signing"].as_object().unwrap_or(&empty);
    let parsing = result["parsing"].as_object().unwrap_or(&empty);
    let network = result["network"]["sdks"].as_object().unwrap_or(&empty);
    let live = result["live"]["sdks"].as_object().unwrap_or(&empty);
    let first = result["options"]["profiles"][0].as_str().unwrap_or("release").to_owned();
    let second = result["options"]["profiles"][1].as_str().map(str::to_owned);
    let mut names: Vec<String> = Vec::new();
    for name in signing.keys().chain(parsing.keys()).chain(network.keys()).chain(live.keys()) {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let us_cell = |value: &Value| value.as_f64().map_or("-".to_owned(), us);
    let ms_cell = |value: &Value| value.as_f64().map_or("-".to_owned(), |ms| format!("{ms:.1}"));
    let rate_cell = |value: &Value| value.as_f64().map_or("-".to_owned(), thousands);
    let mut rows = Vec::new();
    let mut push = |label: String, sign: &Value, parse: &Value, round: Option<&Value>, age: Option<&Value>| {
        let all = &parse["parse"]["all"];
        let quicknode = round.is_some_and(|row| row["route"].is_string());
        let applicable = age.is_some_and(|row| row["not_applicable"].is_null());
        rows.push((
            vec![
                label,
                us_cell(&sign["sign"]["median_ns"]),
                us_cell(&sign["sign"]["p99_ns"]),
                us_cell(&sign["build_sign_body"]["median_ns"]),
                rate_cell(&all["frames_per_s"]),
                all["frame_p99_ns"].as_f64().map_or("-".to_owned(), thousands),
                all["allocations_per_frame"].as_f64().map_or("-".to_owned(), |n| format!("{n:.1}")),
                round.map_or("-".to_owned(), |row| ms_cell(&row["info_summary"]["median_ms"])),
                round.map_or("-".to_owned(), |row| ms_cell(&row["exchange_summary"]["median_ms"])),
                if applicable {
                    age.map_or("-".to_owned(), |row| ms_cell(&row["age_ms_median"]))
                } else {
                    "-".to_owned()
                },
            ],
            quicknode,
        ));
    };
    for name in &names {
        let sign = signing.get(name).unwrap_or(&Value::Null);
        let parse = parsing.get(name).unwrap_or(&Value::Null);
        // Build profiles apply to the Rust rows only; a row names its profile when it has both.
        let other =
            second.as_ref().map(|second| (&sign["profiles"][second.as_str()], &parse["profiles"][second.as_str()]));
        let both = other.is_some_and(|(s2, p2)| !s2.is_null() || !p2.is_null());
        let label = if both { format!("{name}, {first}") } else { name.clone() };
        push(label, sign, parse, network.get(name), live.get(name));
        if let (true, Some(second), Some((s2, p2))) = (both, &second, other) {
            push(format!("{name}, {second}"), s2, p2, None, None);
        }
    }
    let via_quicknode = rows.iter().any(|(_, quicknode)| *quicknode);
    Table {
        title: "All results, one row per SDK and build".to_owned(),
        header: [
            "SDK",
            "hash + sign median (us)",
            "hash + sign p99 (us)",
            "build + sign + body median (us)",
            "parse all frames/s",
            "parse p99 per frame (ns)",
            "allocations per frame",
            "info round trip median (ms)",
            "exchange round trip median (ms)",
            "live frame age median (ms)",
        ]
        .map(str::to_owned)
        .to_vec(),
        rows: rows.into_iter().map(|(cells, _)| (cells, None)).collect(),
        note: Some(format!(
            "The tables below have every percentile, the notes on each test and the reasons for each `-`.{}",
            if via_quicknode {
                " quicknode-hyperliquid-sdk's round trips go through QuickNode's own service, not straight to the venue, so they are not comparable with the other rows."
            } else {
                ""
            }
        )),
    }
}

pub fn markdown(result: &Value) -> String {
    let mut out = vec!["# df-hyperliquid-core benchmark".to_owned(), String::new()];
    for line in heading(result) {
        out.push(format!("{line}  "));
    }
    for table in std::iter::once(summary_table(result)).chain(tables(result)) {
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

/// The Rust competitors, which all sign with k256, the pure-Rust secp256k1 library
/// df-hyperliquid-core uses by default.
pub const PURE_RUST: [&str; 4] = [HYPERSDK, LHERMOSO, QUICKNODE, OFFICIAL_RUST];

/// Whether `name` signed with libsecp256k1 (C) in this run, the library the
/// `df-hyperliquid-core + secp256k1` row uses: go-hyperliquid when it was built with cgo, and the
/// Python SDK with coincurve.
pub fn signs_with_c(result: &Value, name: &str) -> bool {
    match name {
        GO => result["versions"]["go_build"]["cgo"].as_bool() == Some(true),
        PY_FAST => true,
        _ => false,
    }
}

/// The competitors in the share text, by a short name: it has room for at most 280 characters.
const SHORT_NAMES: [(&str, &str); 7] = [
    (HYPERSDK, "hypersdk"),
    (LHERMOSO, "lhermoso"),
    (QUICKNODE, "quicknode"),
    (OFFICIAL_RUST, "official-rs"),
    (NKTKAS, "nktkas"),
    (GO, "go"),
    (PY, "python"),
];

fn figure(ratio: f64) -> String {
    if ratio >= 10.0 { format!("{ratio:.0}") } else { format!("{ratio:.1}") }
}

/// At most 280 characters: df-hyperliquid-core's speed over each competitor's, for parsing and
/// for signing, and the machine. A ratio below 1 means that competitor was faster. Signing
/// compares like with like: an SDK that signs with libsecp256k1 (C) against the `secp256k1` row,
/// when it ran, and every other SDK against the default build (k256, pure Rust).
pub fn summary(result: &Value) -> Result<String> {
    if result["options"]["quick"].as_bool() == Some(true) {
        return Err("a quick run is only a check that the harness runs; run `just bench-compare` for numbers to share"
            .to_owned());
    }
    let sign = |name: &str| f(&result["signing"][name]["sign"]["median_ns"]);
    let parse = |name: &str| f(&result["parsing"][name]["parse"]["all"]["frames_per_s"]);
    let machine = format!(
        "{} ({} {})",
        result["machine"]["cpu"].as_str().unwrap_or("unknown CPU"),
        result["machine"]["os"].as_str().unwrap_or_default(),
        result["machine"]["arch"].as_str().unwrap_or_default()
    );
    let version = |name: &str| result["versions"][name].as_str().unwrap_or("?").to_owned();
    let head = format!("df-hyperliquid-core {} on {machine}", version(DF));
    let short_head = format!(
        "df-hyperliquid-core {} on {}",
        version(DF),
        result["machine"]["cpu"].as_str().unwrap_or("unknown CPU")
    );
    let ratios = |skip_python: bool| -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut parsed = Vec::new();
        let mut signed = Vec::new();
        let mut signed_c = Vec::new();
        for (name, short) in SHORT_NAMES {
            if (skip_python && name == PY)
                || result["signing"].get(name).is_none() && result["parsing"].get(name).is_none()
            {
                continue;
            }
            if let Some(ratio) = ratio(parse(DF), parse(name)) {
                parsed.push(format!("{short} {}", figure(ratio)));
            }
            // The Python SDK's faster backend, libsecp256k1 through coincurve, when it ran.
            let backend = if name == PY && sign(PY_FAST) > 0.0 { PY_FAST } else { name };
            if signs_with_c(result, backend) && sign(DF_SECP) > 0.0 {
                if let Some(ratio) = ratio(sign(backend), sign(DF_SECP)) {
                    signed_c.push(format!("{short} {}", figure(ratio)));
                }
            } else if let Some(ratio) = ratio(sign(backend), sign(DF)) {
                signed.push(format!("{short} {}", figure(ratio)));
            }
        }
        (parsed, signed, signed_c)
    };
    let text = |skip_python: bool, link: bool, short_machine: bool| -> Option<String> {
        let (parsed, signed, signed_c) = ratios(skip_python);
        if parsed.is_empty() && signed.is_empty() && signed_c.is_empty() {
            return None;
        }
        let head = if short_machine { &short_head } else { &head };
        let mut text = format!("{head}. df speed / theirs, below 1 = slower.");
        if !parsed.is_empty() {
            text.push_str(&format!(" Parse: {}.", parsed.join(", ")));
        }
        if !signed.is_empty() {
            text.push_str(&format!(" Sign, k256: {}.", signed.join(", ")));
        }
        if !signed_c.is_empty() {
            text.push_str(&format!(" Sign, libsecp256k1: {}.", signed_c.join(", ")));
        }
        if link {
            text.push_str(" github.com/dennisfurrer/df-hyperliquid-core/tree/main/bench");
        }
        Some(text)
    };
    let Some(full) = text(false, true, false) else {
        return Ok(format!(
            "{head}, df-hyperliquid-core only, no comparison: sign {} us, parse {} frames/s. github.com/dennisfurrer/df-hyperliquid-core/tree/main/bench",
            us(sign(DF)),
            thousands(parse(DF))
        ));
    };
    // Drop the link first, then the Python figures, then the OS, to stay within one post.
    let mut chosen = full;
    for (skip_python, link, short_machine) in [(false, false, false), (true, false, false), (true, false, true)] {
        if chosen.chars().count() <= 280 {
            break;
        }
        chosen = text(skip_python, link, short_machine).unwrap_or_default();
    }
    Ok(chosen.chars().take(280).collect())
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

    use super::{markdown, summary};
    use crate::offline::{COMPETITORS, DF, DF_SECP, GO, HYPERSDK, PY, PY_FAST, QUICKNODE};

    fn result(rows: &[&str]) -> Value {
        let mut result = json!({
            "options": { "quick": false },
            "machine": { "cpu": "Apple M5 Max", "os": "macOS 26.5", "arch": "aarch64" },
            "versions": { DF: "0.2.5" },
            "signing": {},
            "parsing": {},
        });
        for name in rows {
            result["versions"][*name] = json!("0.1.0");
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
    fn a_run_without_hypersdk_still_compares() {
        let text = summary(&result(&[DF, PY])).unwrap();
        assert!(text.contains("python"), "{text}");
        assert!(!text.contains("hypersdk"), "{text}");
    }

    #[test]
    fn a_compare_run_with_every_competitor_fits_one_post() {
        let mut rows = vec![DF];
        rows.extend(COMPETITORS.iter().filter(|name| **name != PY_FAST));
        let mut result = result(&rows);
        // Slow Python figures, the longest ratios.
        result["parsing"][PY]["parse"]["all"]["frames_per_s"] = json!(15_000.0);
        let text = summary(&result).unwrap();
        assert!(text.contains("hypersdk 1.0") && text.contains("go 1.0"), "{text}");
        assert!(text.chars().count() <= 280, "{} characters: {text}", text.chars().count());
    }

    #[test]
    fn a_faster_competitor_shows_a_ratio_below_one() {
        let mut result = result(&[DF, HYPERSDK, GO]);
        result["signing"][GO]["sign"]["median_ns"] = json!(15_000.0);
        let text = summary(&result).unwrap();
        assert!(text.contains("Sign, k256: hypersdk 1.0, go 0.5"), "{text}");
    }

    #[test]
    fn signing_is_compared_like_with_like() {
        let mut result = result(&[DF, HYPERSDK, GO, PY]);
        result["versions"]["go_build"] = json!({ "cgo": true });
        result["signing"][DF_SECP] = json!({ "sign": { "median_ns": 10_000.0 } });
        result["signing"][GO]["sign"]["median_ns"] = json!(15_000.0);
        let text = summary(&result).unwrap();
        assert!(text.contains("Sign, k256: hypersdk 1.0."), "{text}");
        assert!(text.contains("Sign, libsecp256k1: go 1.5, python 9.0."), "{text}");
    }

    #[test]
    fn quicknode_without_an_endpoint_is_not_applicable_live_and_not_ranked_in_round_trips() {
        let mut result = result(&[DF, QUICKNODE]);
        let summary = json!({ "median_ms": 300.0, "p90_ms": 400.0, "p99_ms": 500.0 });
        result["network"] = json!({ "rounds": 100, "sdks": {
            DF: { "info_summary": summary, "info_failed": 0, "exchange_summary": summary, "exchange_failed": 0 },
            QUICKNODE: { "info_summary": summary, "info_failed": 0, "exchange_summary": summary, "exchange_failed": 0,
                "route": "QuickNode's own service", "ranked": false },
        }});
        result["live"] = json!({ "seconds": 60, "sdks": {
            DF: { "book_frames": 100, "bbo_frames": 400, "trade_frames": 60, "unreadable": 0, "age_ms_median": 260, "age_ms_p99": 600 },
            QUICKNODE: { "not_applicable": "its Stream needs a QuickNode endpoint, and none was set (QUICKNODE_ENDPOINT)" },
        }});
        let text = markdown(&result);
        assert!(text.contains("| quicknode-hyperliquid-sdk | n/a | n/a | n/a | n/a | n/a | n/a |"), "{text}");
        assert!(text.contains("quicknode-hyperliquid-sdk: not applicable"), "{text}");
        assert!(!text.contains("quicknode-hyperliquid-sdk have no fast l2Book"), "{text}");
        assert!(text.contains("| quicknode-hyperliquid-sdk, via QuickNode | 300.0 |"), "{text}");
        assert!(text.contains("is not ranked with the others"), "{text}");
    }

    #[test]
    fn a_run_with_both_profiles_shows_the_second_in_its_own_columns() {
        let mut result = result(&[DF, GO]);
        result["options"]["profiles"] = json!(["release", "release-lto"]);
        result["signing"][DF]["profiles"]["release-lto"] = json!({ "sign": { "median_ns": 24_000.0 } });
        result["parsing"][DF]["profiles"]["release-lto"] = json!({ "parse": { "all": { "frames_per_s": 600_000.0 } } });
        let text = markdown(&result);
        assert!(
            text.contains("Rust build profiles: release (cargo's default release profile) in the main columns"),
            "{text}"
        );
        assert!(text.contains("hash + sign median, release-lto (us)"), "{text}");
        assert!(text.contains("all frames/s, release-lto"), "{text}");
        assert!(text.contains("| 24.0 | - |"), "{text}");
        assert!(text.contains("| 600,000 |"), "{text}");
        // go-hyperliquid is not a Rust row, so it has no second-profile figures.
        assert!(text.contains("| go-hyperliquid | 30.0 | 0.0 | 0.0 | 0.0 | - | - |"), "{text}");
    }

    #[test]
    fn the_summary_table_has_one_row_per_sdk_and_build_with_numbers_only() {
        let mut result = result(&[DF, GO]);
        result["options"]["profiles"] = json!(["release", "release-lto"]);
        result["signing"][DF]["profiles"]["release-lto"] = json!({ "sign": { "median_ns": 24_000.0 } });
        let text = markdown(&result);
        let at = text.find("## All results, one row per SDK and build").expect("the summary table");
        assert!(at < text.find("## Signing").expect("the signing table"), "{text}");
        assert!(text.contains("| df-hyperliquid-core, release | 30.0 |"), "{text}");
        assert!(text.contains("| df-hyperliquid-core, release-lto | 24.0 |"), "{text}");
        assert!(text.contains("| go-hyperliquid | 30.0 |"), "{text}");
        assert!(!text.contains("| go-hyperliquid, release-lto |"), "{text}");
    }
}
