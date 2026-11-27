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

/// `lscpu`'s value for `key`, when it names something (ARM Linux often reports `-`).
fn lscpu(key: &str) -> Option<String> {
    let output = std::process::Command::new("lscpu").output().ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.split(':').next().is_some_and(|name| name.trim() == key))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty() && value != "-")
}

fn cpu() -> String {
    let named = |value: Option<String>| value.filter(|value| !value.is_empty() && value != "-");
    let found = match std::env::consts::OS {
        "macos" => named(mac_sysctl("machdep.cpu.brand_string")),
        "linux" => named(linux_field("/proc/cpuinfo", "model name"))
            .or_else(|| lscpu("Model name"))
            .or_else(|| lscpu("Vendor ID").map(|vendor| format!("{vendor} {}", std::env::consts::ARCH))),
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
        "linux" => std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("PRETTY_NAME=").map(|name| name.trim_matches('"').to_owned()))
            })
            .unwrap_or_else(|| "Linux".to_owned()),
        other => other.to_owned(),
    }
}

/// `pmset -g batt` says `Now drawing from 'AC Power'` or `'Battery Power'`.
fn mac_power_source(batt: &str) -> Option<String> {
    let line = batt.lines().find(|line| line.contains("drawing from"))?;
    Some(line.split('\'').nth(1)?.to_owned())
}

/// The energy mode in `pmset -g`. Macs with High Power Mode report `powermode` (0 automatic,
/// 1 low power, 2 high power); others report `lowpowermode` (0 or 1).
fn mac_power_mode(settings: &str) -> Option<String> {
    let value = |key: &str| {
        settings.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some(key)).then(|| words.next().map(str::to_owned)).flatten()
        })
    };
    let mode = match (value("powermode").as_deref(), value("lowpowermode").as_deref()) {
        (Some("0"), _) | (None, Some("0")) => "automatic",
        (Some("1"), _) | (None, Some("1")) => "low power",
        (Some("2"), _) => "high power",
        (Some(other), _) | (None, Some(other)) => return Some(format!("unknown ({other})")),
        (None, None) => return None,
    };
    Some(mode.to_owned())
}

/// On Linux: AC when a mains supply is online, battery when a battery is discharging, and the
/// ACPI platform profile (`low-power`, `balanced`, `performance`) as the mode.
fn linux_power() -> (Option<String>, Option<String>) {
    let read = |path: std::path::PathBuf| std::fs::read_to_string(path).ok().map(|text| text.trim().to_owned());
    let mut source = None;
    for entry in std::fs::read_dir("/sys/class/power_supply").into_iter().flatten().flatten() {
        let path = entry.path();
        match read(path.join("type")).as_deref() {
            Some("Mains") if read(path.join("online")).as_deref() == Some("1") => source = Some("AC Power".to_owned()),
            Some("Battery") if read(path.join("status")).as_deref() == Some("Discharging") && source.is_none() => {
                source = Some("Battery Power".to_owned());
            }
            _ => {}
        }
    }
    (source, read("/sys/firmware/acpi/platform_profile".into()))
}

/// Where the power comes from and the energy mode, when the OS says. Laptops on battery or in
/// low power mode run slower and less steadily, so both go into the results.
pub fn power() -> Value {
    let (source, mode) = match std::env::consts::OS {
        "macos" => {
            let output = |args: &[&str]| {
                std::process::Command::new("pmset")
                    .args(args)
                    .output()
                    .ok()
                    .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            };
            (
                output(&["-g", "batt"]).as_deref().and_then(mac_power_source),
                output(&["-g"]).as_deref().and_then(mac_power_mode),
            )
        }
        "linux" => linux_power(),
        _ => (None, None),
    };
    json!({ "source": source, "mode": mode })
}

/// Why this power state may slow the run down, if it may.
pub fn power_warning(power: &Value) -> Option<String> {
    let on_battery = power["source"].as_str() == Some("Battery Power");
    let low = power["mode"].as_str().is_some_and(|mode| mode == "low power" || mode == "low-power");
    let state = match (on_battery, low) {
        (true, true) => "running on battery in low power mode: plug in and turn low power mode off",
        (true, false) => "running on battery: plug in",
        (false, true) => "running in low power mode: turn it off",
        (false, false) => return None,
    };
    Some(format!("{state}; results are slower and less steady otherwise"))
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
    let slug = slug.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-').to_owned();
    if slug.is_empty() { "unknown-cpu".to_owned() } else { slug }
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
        "power": power(),
        // What the benchmark programs ask the scheduler for (see `bench_common::prefer_performance_cores`).
        "qos": if std::env::consts::OS == "macos" { Value::from("user-interactive") } else { Value::Null },
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{mac_power_mode, mac_power_source, power_warning, slug};

    #[test]
    fn a_cpu_name_becomes_a_file_name() {
        assert_eq!(slug("Apple M5 Max"), "apple-m5-max");
        assert_eq!(slug("AMD Ryzen 9 7950X 16-Core Processor"), "amd-ryzen-9-7950x-16-core-processor");
        assert_eq!(slug("Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz"), "intel-xeon-platinum-8375c-cpu-2-90ghz");
        assert_eq!(slug("-"), "unknown-cpu");
    }

    #[test]
    fn the_power_state_is_read_from_pmset() {
        let batt = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t63%; discharging;";
        assert_eq!(mac_power_source(batt).as_deref(), Some("Battery Power"));
        assert_eq!(mac_power_mode(" sleep 1\n powermode            2\n").as_deref(), Some("high power"));
        assert_eq!(mac_power_mode(" lowpowermode         1\n").as_deref(), Some("low power"));
        assert_eq!(mac_power_mode(" sleep 1\n"), None);
    }

    #[test]
    fn battery_and_low_power_mode_are_warned_about() {
        assert!(power_warning(&json!({ "source": "AC Power", "mode": "automatic" })).is_none());
        assert!(power_warning(&json!({ "source": "Battery Power", "mode": "high power" })).is_some());
        assert!(power_warning(&json!({ "source": "AC Power", "mode": "low power" })).is_some());
        assert!(power_warning(&json!({ "source": null, "mode": null })).is_none());
    }
}
