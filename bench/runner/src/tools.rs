//! Which tools are installed, what each one enables, and `doctor`, which prints both.

use std::path::Path;
use std::process::Command;

/// What was found on this machine. `None` is missing.
pub struct Tools {
    pub rustc: Option<String>,
    pub c_compiler: Option<String>,
    pub uv: Option<String>,
    pub node: Option<String>,
    pub pnpm: Option<String>,
    pub go: Option<String>,
    pub protoc: Option<String>,
    pub just: Option<String>,
    pub git: Option<String>,
}

/// The first line a tool prints for `args`, or `None` when it does not run.
pub fn first_line(program: &str, args: &[&str], cwd: &Path) -> Option<String> {
    let output = Command::new(program).args(args).current_dir(cwd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = if output.stdout.is_empty() { output.stderr } else { output.stdout };
    String::from_utf8_lossy(&text).lines().next().map(|line| line.trim().to_owned())
}

pub fn find(bench: &Path) -> Tools {
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_owned());
    Tools {
        rustc: first_line("rustc", &["--version"], bench),
        c_compiler: first_line(&compiler, &["--version"], bench).map(|line| format!("{compiler}: {line}")),
        uv: first_line("uv", &["--version"], bench),
        node: first_line("node", &["--version"], bench),
        pnpm: first_line("pnpm", &["--version"], bench),
        go: first_line("go", &["version"], bench),
        protoc: first_line("protoc", &["--version"], bench),
        just: first_line("just", &["--version"], bench),
        git: first_line("git", &["--version"], bench),
    }
}

/// The crate's minimum Rust version, from the workspace manifest.
pub fn msrv(bench: &Path) -> Option<String> {
    let manifest = std::fs::read_to_string(bench.join("../Cargo.toml")).ok()?;
    let line = manifest.lines().find(|line| line.trim_start().starts_with("rust-version"))?;
    line.split('"').nth(1).map(str::to_owned)
}

/// `rustc 1.94.1 (...)` as numbers.
fn rustc_numbers(version: &str) -> Vec<u64> {
    version.split_whitespace().nth(1).unwrap_or_default().split('.').filter_map(|part| part.parse().ok()).collect()
}

pub fn doctor(bench: &Path) {
    let found = find(bench);
    let need = msrv(bench).unwrap_or_else(|| "unknown".to_owned());
    let wanted: Vec<u64> = need.split('.').filter_map(|part| part.parse().ok()).collect();
    let line =
        |ok: bool, name: &str, detail: &str| println!("{} {name:<12} {detail}", if ok { "ok     " } else { "missing" });

    match &found.rustc {
        Some(version) if rustc_numbers(version) >= wanted => {
            line(true, "rustc", &format!("{version}; needs {need} or newer"));
        }
        Some(version) => line(false, "rustc", &format!("{version} is older than {need}; run `rustup update`")),
        None => line(false, "rustc", "install Rust from https://rustup.rs; needed for everything"),
    }
    match &found.c_compiler {
        Some(compiler) => line(true, "C compiler", &format!("{compiler}; builds the secp256k1 row")),
        None => line(false, "C compiler", "the df-hyperliquid-core + secp256k1 row is skipped"),
    }
    match &found.uv {
        Some(version) => line(true, "uv", &format!("{version}; runs the Python SDK rows in compare mode")),
        None => line(false, "uv", "the Python SDK rows are skipped; install from https://docs.astral.sh/uv/"),
    }
    match &found.node {
        Some(version) => line(
            true,
            "node",
            &format!("{version}; runs the @nktkas/hyperliquid rows in compare mode (needs 22.12 or newer)"),
        ),
        None => line(false, "node", "the @nktkas/hyperliquid rows are skipped; install Node 22.12 or newer"),
    }
    match &found.pnpm {
        Some(version) => {
            line(true, "pnpm", &format!("{version}; installs @nktkas/hyperliquid from bench/ts/pnpm-lock.yaml"))
        }
        None => line(false, "pnpm", "the @nktkas/hyperliquid rows are skipped; install from https://pnpm.io"),
    }
    match &found.go {
        Some(version) => line(true, "go", &format!("{version}; builds the go-hyperliquid rows in compare mode")),
        None => line(false, "go", "the go-hyperliquid rows are skipped; install from https://go.dev/dl/"),
    }
    match &found.protoc {
        Some(version) => line(true, "protoc", &format!("{version}; quicknode-hyperliquid-sdk's build script needs it")),
        None => line(
            false,
            "protoc",
            "the quicknode-hyperliquid-sdk rows are skipped; install protobuf (brew install protobuf)",
        ),
    }
    match &found.just {
        Some(version) => line(true, "just", &format!("{version}; optional, every command also runs with cargo")),
        None => line(false, "just", "optional: use the cargo commands in bench/README.md"),
    }
    match &found.git {
        Some(version) => line(true, "git", &format!("{version}; records the commit in the results")),
        None => line(false, "git", "the results record no commit"),
    }
}
