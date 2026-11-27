//! The machine a run is on: CPU, cores, memory, OS and architecture. No host name or user name:
//! results are meant to be committed and shared.

use std::path::Path;

use serde_json::{Value, json};

use crate::tools::first_line;

fn mac_sysctl(name: &str) -> Option<String> {
    first_line("sysctl", &["-n", name], Path::new("."))
}

fn linux_field(file: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    text.lines()
        .find(|line| line.split(':').next().is_some_and(|name| name.trim() == key))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_owned())
}

fn cpu() -> String {
    let found = match std::env::consts::OS {
        "macos" => mac_sysctl("machdep.cpu.brand_string"),
        "linux" => linux_field("/proc/cpuinfo", "model name").or_else(|| {
            let lscpu = std::process::Command::new("lscpu").output().ok()?;
            String::from_utf8_lossy(&lscpu.stdout)
                .lines()
                .find(|line| line.starts_with("Model name"))
                .and_then(|line| line.split_once(':'))
                .map(|(_, value)| value.trim().to_owned())
        }),
        _ => None,
    };
    found.unwrap_or_else(|| format!("unknown {} CPU", std::env::consts::ARCH))
}

fn memory_gb() -> Option<u64> {
    match std::env::consts::OS {
        "macos" => mac_sysctl("hw.memsize")?.parse::<u64>().ok().map(|bytes| bytes >> 30),
        "linux" => {
            let kb = linux_field("/proc/meminfo", "MemTotal")?;
            kb.split_whitespace().next()?.parse::<u64>().ok().map(|kb| kb >> 20)
        }
        _ => None,
    }
}

fn os() -> String {
    match std::env::consts::OS {
        "macos" => {
            let name = first_line("sw_vers", &["-productName"], Path::new(".")).unwrap_or_else(|| "macOS".to_owned());
            let version = first_line("sw_vers", &["-productVersion"], Path::new(".")).unwrap_or_default();
            format!("{name} {version}").trim().to_owned()
        }
        "linux" => linux_field("/etc/os-release", "PRETTY_NAME")
            .map(|name| name.trim_matches('"').to_owned())
            .unwrap_or_else(|| "Linux".to_owned()),
        other => other.to_owned(),
    }
}

/// `Apple M5 Max` as `apple-m5-max`, for the results file name.
pub fn slug(cpu: &str) -> String {
    let mut slug = String::new();
    for c in cpu.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.replace("-r-", "-").replace("-tm-", "-");
    slug.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-').to_owned()
}

pub fn describe() -> Value {
    let cpu = cpu();
    json!({
        "cpu": cpu,
        "cpu_slug": slug(&cpu),
        "cores": std::thread::available_parallelism().map(usize::from).ok(),
        "memory_gb": memory_gb(),
        "os": os(),
        "arch": std::env::consts::ARCH,
    })
}
