//! What the two Rust benchmark binaries share: the fixed signing input, the timing harnesses,
//! the summary statistics and the recorded websocket corpus. Nothing here depends on an SDK.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::hint::black_box;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde_json::{Value, json};

/// A throwaway key, the example key from the web3.js documentation. It is public and holds
/// nothing. Used only for the offline signing benchmark and the signature check.
pub const FIXED_KEY: &str = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";
/// The nonce of the signed order (milliseconds).
pub const FIXED_NONCE: u64 = 1_700_000_000_000;
/// BTC is perp asset 0 on the main dex.
pub const FIXED_ASSET: u32 = 0;
pub const FIXED_PX: &str = "50000";
pub const FIXED_SZ: &str = "0.001";

/// Untimed iterations before the signing samples.
pub const SIGN_WARMUP: usize = 2_000;
/// Timed signing iterations per SDK.
pub const SIGN_SAMPLES: usize = 20_000;

/// Untimed passes over the corpus, then timed passes.
pub const PARSE_WARMUP_PASSES: usize = 3;
pub const PARSE_PASSES: usize = 20;

/// Whether the runner asked for a quick run (`BENCH_QUICK=1`): a tenth of the signing samples
/// and a quarter of the parse passes, to check that everything runs rather than to measure.
#[must_use]
pub fn quick() -> bool {
    std::env::var("BENCH_QUICK").is_ok_and(|value| value == "1")
}

/// [`SIGN_WARMUP`], or a tenth of it in a quick run.
#[must_use]
pub fn sign_warmup() -> usize {
    if quick() { SIGN_WARMUP / 10 } else { SIGN_WARMUP }
}

/// [`SIGN_SAMPLES`], or a tenth of it in a quick run.
#[must_use]
pub fn sign_samples() -> usize {
    if quick() { SIGN_SAMPLES / 10 } else { SIGN_SAMPLES }
}

fn parse_warmup_passes() -> usize {
    if quick() { 1 } else { PARSE_WARMUP_PASSES }
}

fn parse_passes() -> usize {
    if quick() { PARSE_PASSES / 4 } else { PARSE_PASSES }
}

/// The system allocator, counting every allocation and reallocation. Each Rust binary installs it
/// as its global allocator, so the parse report can give allocations per frame. The count is one
/// relaxed atomic add a call, the same for both SDKs.
pub struct CountingAlloc;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

// SAFETY: every call goes straight to `System` with the caller's arguments.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Allocations and reallocations so far, when [`CountingAlloc`] is the global allocator.
#[must_use]
pub fn allocations() -> u64 {
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// Asks macOS to run the calling thread at the user-interactive quality of service, which makes
/// the scheduler much more likely to keep it on a performance core. macOS does not pass this on
/// to threads the caller starts later, so each thread that does timed work calls it. Returns
/// whether the class was set; elsewhere it does nothing and returns `false`.
#[cfg(target_os = "macos")]
pub fn prefer_performance_cores() -> bool {
    // From <pthread/qos.h>: QOS_CLASS_USER_INTERACTIVE.
    const USER_INTERACTIVE: u32 = 0x21;
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(class: u32, relative_priority: i32) -> i32;
    }
    // SAFETY: the call only changes the scheduling class of the calling thread.
    unsafe { pthread_set_qos_class_self_np(USER_INTERACTIVE, 0) == 0 }
}

/// Does nothing outside macOS.
#[cfg(not(target_os = "macos"))]
pub fn prefer_performance_cores() -> bool {
    false
}

/// A multi-threaded tokio runtime whose threads, and the calling thread, ask for performance
/// cores first (see [`prefer_performance_cores`]).
///
/// # Errors
/// When tokio cannot start its threads.
pub fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    prefer_performance_cores();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_start(|| {
            prefer_performance_cores();
        })
        .build()
}

/// Unix milliseconds from the system clock.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

/// Times `run` once per call, `count` times, and returns the nanoseconds of each call.
pub fn time_each<T>(count: usize, mut run: impl FnMut() -> T) -> Vec<u64> {
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let start = Instant::now();
        black_box(run());
        samples.push(start.elapsed().as_nanos() as u64);
    }
    samples
}

/// Runs `run` `count` times without timing it.
pub fn warm<T>(count: usize, mut run: impl FnMut() -> T) {
    for _ in 0..count {
        black_box(run());
    }
}

fn percentile(sorted: &[u64], fraction: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// Count, min, median, p90, p99, max and mean of nanosecond samples, and the rate one call at
/// the mean time gives.
#[must_use]
pub fn summary_ns(samples: &[u64]) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let total: u128 = sorted.iter().map(|&sample| u128::from(sample)).sum();
    let mean = if sorted.is_empty() { 0.0 } else { total as f64 / sorted.len() as f64 };
    json!({
        "n": sorted.len(),
        "min_ns": sorted.first().copied().unwrap_or(0),
        "median_ns": percentile(&sorted, 0.5),
        "p90_ns": percentile(&sorted, 0.9),
        "p99_ns": percentile(&sorted, 0.99),
        "max_ns": sorted.last().copied().unwrap_or(0),
        "mean_ns": mean,
        "ops_per_s": if mean > 0.0 { 1e9 / mean } else { 0.0 },
    })
}

/// One recorded websocket text frame and the channel it named.
pub struct RecordedFrame {
    pub channel: String,
    pub text: String,
}

/// Reads the gzipped JSON-lines corpus written by `py/record.py`: one text frame a line.
pub fn load_corpus(path: &str) -> Result<Vec<RecordedFrame>, String> {
    let file = File::open(path).map_err(|error| format!("cannot open {path}: {error}"))?;
    let mut frames = Vec::new();
    for line in BufReader::new(GzDecoder::new(file)).lines() {
        let text = line.map_err(|error| format!("cannot read {path}: {error}"))?;
        if text.is_empty() {
            continue;
        }
        let channel = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| value.get("channel").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        frames.push(RecordedFrame { channel, text });
    }
    Ok(frames)
}

/// Decodes every frame of `frames` with `decode` ([`PARSE_WARMUP_PASSES`] untimed passes, then
/// [`PARSE_PASSES`] timed ones, fewer in a quick run) and reports frames/s and MB/s over the timed passes. `decode`
/// answers whether the frame read into the SDK's type.
pub fn parse_throughput(frames: &[&RecordedFrame], mut decode: impl FnMut(&str) -> bool) -> Value {
    let bytes: usize = frames.iter().map(|frame| frame.text.len()).sum();
    let failed = frames.iter().filter(|frame| !decode(&frame.text)).count();
    let passes_count = parse_passes();
    for _ in 0..parse_warmup_passes() {
        for frame in frames {
            black_box(decode(black_box(&frame.text)));
        }
    }
    let mut passes = Vec::with_capacity(passes_count);
    for _ in 0..passes_count {
        let start = Instant::now();
        for frame in frames {
            black_box(decode(black_box(&frame.text)));
        }
        passes.push(start.elapsed());
    }
    let total: Duration = passes.iter().sum();
    let seconds = total.as_secs_f64();
    let mut pass_ns: Vec<u64> = passes.iter().map(|pass| pass.as_nanos() as u64).collect();
    pass_ns.sort_unstable();
    // One more pass, each frame timed on its own, for the spread across frames.
    let mut frame_ns: Vec<u64> = frames
        .iter()
        .map(|frame| {
            let start = Instant::now();
            black_box(decode(black_box(&frame.text)));
            start.elapsed().as_nanos() as u64
        })
        .collect();
    frame_ns.sort_unstable();
    // And one counted pass.
    let before = allocations();
    for frame in frames {
        black_box(decode(black_box(&frame.text)));
    }
    let allocated = allocations() - before;
    json!({
        "frames": frames.len(),
        "bytes": bytes,
        "failed": failed,
        "passes": passes_count,
        "frames_per_s": (frames.len() * passes_count) as f64 / seconds,
        "mb_per_s": (bytes * passes_count) as f64 / seconds / 1e6,
        "median_pass_ns": percentile(&pass_ns, 0.5),
        "ns_per_frame": seconds * 1e9 / (frames.len() * passes_count) as f64,
        "frame_median_ns": percentile(&frame_ns, 0.5),
        "frame_p99_ns": percentile(&frame_ns, 0.99),
        "allocations_per_frame": allocated as f64 / frames.len().max(1) as f64,
    })
}

/// [`parse_throughput`] over the whole corpus, then over its `l2Book` and its `trades` frames
/// alone.
pub fn parse_report(corpus: &[RecordedFrame], mut decode: impl FnMut(&str) -> bool) -> Value {
    let all: Vec<&RecordedFrame> = corpus.iter().collect();
    let books: Vec<&RecordedFrame> = corpus.iter().filter(|frame| frame.channel == "l2Book").collect();
    let trades: Vec<&RecordedFrame> = corpus.iter().filter(|frame| frame.channel == "trades").collect();
    json!({
        "all": parse_throughput(&all, &mut decode),
        "l2Book": parse_throughput(&books, &mut decode),
        "trades": parse_throughput(&trades, &mut decode),
    })
}

/// The command line after the binary's name.
#[must_use]
pub fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// The value after `--name`, if given.
#[must_use]
pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).cloned()
}

/// What a live websocket run reports: frames delivered to the program, by channel, frames the
/// SDK could not read, and how old each book or trade was, by the venue's own timestamp, when
/// the program got it as a typed value.
#[derive(Default)]
pub struct LiveTally {
    pub book_frames: u64,
    pub bbo_frames: u64,
    pub trade_frames: u64,
    pub trades: u64,
    pub other_frames: u64,
    pub unreadable: u64,
    pub connects: u64,
    pub age_ms: Vec<u64>,
}

impl LiveTally {
    /// Records a delivered frame stamped `venue_ms` by the venue.
    pub fn aged(&mut self, venue_ms: u64) {
        self.age_ms.push(now_ms().saturating_sub(venue_ms));
    }

    #[must_use]
    pub fn report(&self, seconds: u64) -> Value {
        let mut sorted = self.age_ms.clone();
        sorted.sort_unstable();
        json!({
            "seconds": seconds,
            "book_frames": self.book_frames,
            "bbo_frames": self.bbo_frames,
            "trade_frames": self.trade_frames,
            "trades": self.trades,
            "other_frames": self.other_frames,
            "unreadable": self.unreadable,
            "connects": self.connects,
            "age_ms_median": percentile(&sorted, 0.5),
            "age_ms_p99": percentile(&sorted, 0.99),
            "age_ms": self.age_ms,
        })
    }
}
