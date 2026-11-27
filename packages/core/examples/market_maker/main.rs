//! A market maker on one perp: two post-only quotes around the mid from the live touch, skewed by
//! the position, requoted as one `batchModify` over the websocket when a price moves a tick or
//! more, with a position limit, a loss limit, the dead man's switch, cancel on exit and a
//! watchdog. It reports where the time goes for each requote: reading the frame, the quote logic,
//! signing, and the whole from the frame's arrival to a signed request ready to send.
//!
//! Without `HL_SEND` it reads the mainnet touch (no account needed), works out and signs every
//! requote for testnet, and sends nothing; without `HL_PRIVATE_KEY` it signs with a throwaway key
//! made in memory. With `HL_SEND=1` it trades on testnet; on mainnet only with `HL_MAINNET=1` too.
//! With `HL_SEND=1` the book feed and the orders are always on the same network: it prints both at
//! the start and refuses to run if they differ.
//!
//! Environment:
//! - `HL_COIN`: the perp, `BTC` by default.
//! - `HL_SEND=1`: send orders. `HL_MAINNET=1`: send them on mainnet instead of testnet.
//! - `HL_PRIVATE_KEY_FILE`: a file holding the signing key as hex (an API wallet's key is best),
//!   so the key never appears on a command line or in the shell's history. `HL_PRIVATE_KEY` takes
//!   the key itself instead. One of them is needed with `HL_SEND`.
//! - `HL_ADDRESS`: the account traded, when the key is an API wallet's; the key's own address
//!   otherwise.
//! - `HL_SIZE`: each quote's size in the coin; by default the venue's $10 minimum.
//! - `HL_SPREAD_BPS`: half the spread, in basis points of the mid (default 5; 0 for none).
//! - `HL_BEHIND_TICKS`: how many ticks each quote stays behind the best price on its own side
//!   (default 1). `HL_SPREAD_BPS=0 HL_BEHIND_TICKS=0` quotes at the best bid and ask, still
//!   post-only, which suits a thin testnet book.
//! - `HL_SKEW_BPS`: how far the centre moves at the full position, in basis points (default 5).
//! - `HL_MAX_POSITION_USD`: the largest position either way, in USD at the first mid (default 50).
//! - `HL_MAX_LOSS_USD`: stop and cancel everything at this loss (default 5).
//! - `HL_BOOK`: `bbo` (default) or `fast` (the fast `l2Book`).
//! - `HL_MIN_REQUOTE_MS`: the shortest gap between requotes (default 500).
//! - `HL_WATCHDOG_SECS`: cancel everything after this long without a book update or an answer
//!   (default 10). `HL_REPORT_SECS`: how often to print the figures (default 10).
//! - `HL_SECS`: stop after this many seconds (default: run until Ctrl-C).
//! - `HL_SPIN=1`: keep the quoting thread busy between frames instead of letting it sleep, and
//!   sign a throwaway digest (never sent) each millisecond it is idle. macOS moves a thread that
//!   sleeps between frames to a slower core or clock, and its caches go cold, which roughly
//!   doubles the signing time; spinning keeps the requote path as fast as the benchmark's tight
//!   loop, at the cost of one core at 100%. Off by default.
//! - `HL_REQUOTE_AFTER_SWITCH=1`: quote again after the dead man's switch has fired; by default
//!   the program stops.
//!
//! The dead man's switch (`scheduleCancel`) is tried once at the start. If the venue refuses it,
//! it is never sent again in that run, and the watchdog and the cancels on exit are what remain.
//! If it is accepted, it is refreshed on a timer, every 5 seconds to 15 seconds ahead, never per
//! requote: each refresh is an action that spends the address's request budget. A refused
//! refresh (after the day's 10 triggers, for example) stops the refreshing the same way. When the
//! switch fires, the venue cancels every order and `orderUpdates` reports `scheduledCancel`: the
//! program forgets those quotes, so it never modifies an order that is gone, and stops, or with
//! `HL_REQUOTE_AFTER_SWITCH=1` says so and places new quotes.
//!
//! When the venue answers "User or API Wallet 0x... does not exist", it prints the likely causes,
//! every input of the hash and each stage of the signature, from the crate's `Breakdown`, with
//! the address the signature recovers to beside the key's own. It never prints the key.
//!
//! Requests and the rate limit: orders never wait, reads do. Requotes go out over the websocket
//! as soon as they are signed; the dead man's switch and the cancels on exit go over HTTP through
//! a client that sends no reads, so nothing a read does holds them. Every REST read (the startup
//! snapshots, the request budget, the open orders on exit) goes through `reads::read`, which
//! yields first, holds the read while the program's last minute of HTTP weight is over half the
//! venue's 1,200 a minute, and after a 429 holds every read until the cooldown ends. Under that,
//! the crate's own gate holds reads at four fifths of the limit.
//!
//! `cargo run -p df-hyperliquid-core --release --example market_maker`
//! `HL_SEND=1 HL_PRIVATE_KEY_FILE=$HOME/.hl/testnet.key cargo run -p df-hyperliquid-core --release --example market_maker`

mod book;
mod cores;
mod inventory;
mod quote;
mod reads;
mod refusal;
mod safety;
mod stats;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::time::{Duration, Instant};

use df_hyperliquid_core::account::{self, LocalKey, Signature};
use df_hyperliquid_core::client::native::{Feed, Native, clock};
use df_hyperliquid_core::client::{Accepted, ApiError, ExchangeResponse, exchange};
use df_hyperliquid_core::live::{self, Answer, Event, Frame, Post, Session, Subscription};
use df_hyperliquid_core::markets::{Dex, Side, perps};
use df_hyperliquid_core::trade::answer::orders;
use df_hyperliquid_core::trade::{
    Asset, BatchModify, Cancel, CancelByCloid, Cloid, L1Action, Modify, Order, OrderAnswer, OrderRef, PlaceOrder,
    ScheduleCancel, Tif,
};
use df_hyperliquid_core::utils::rounding::Grid;
use df_hyperliquid_core::utils::sign::Breakdown;
use df_hyperliquid_core::{Address, Client, Dec, Network};
use zeroize::Zeroizing;

use book::Source;
use inventory::Inventory;
use quote::{Level, Quotes, Settings};
use safety::{Budget, SWITCH_AHEAD_MS, SWITCH_EVERY, Watchdog};
use stats::Stats;

type Failure = Box<dyn Error>;

/// How often an idle quoting thread signs a throwaway digest with `HL_SPIN=1`.
const WARM_EVERY: Duration = Duration::from_millis(1);

/// The venue's words for a post-only order that would have crossed: normal in a fast market.
const WOULD_CROSS: [&str; 2] = ["Post only order would have immediately matched", "badAloPx"];

/// The settings read from the environment.
struct Config {
    coin: String,
    send: bool,
    trade_net: Network,
    data_net: Network,
    size: Dec,
    half_spread_bps: Dec,
    behind_ticks: Dec,
    skew_bps: Dec,
    max_position_usd: Dec,
    max_loss_usd: Dec,
    source: Source,
    min_requote: Duration,
    watchdog: Duration,
    report_every: Duration,
    run_for: Option<Duration>,
    requote_after_switch: bool,
    spin: bool,
}

impl Config {
    fn read() -> Result<Config, Failure> {
        let flag = |name: &str| env::var(name).is_ok_and(|value| value == "1");
        let dec = |name: &str, default: &str| -> Result<Dec, Failure> {
            Ok(env::var(name)
                .unwrap_or_else(|_| default.to_owned())
                .parse::<Dec>()
                .map_err(|error| format!("{name}: {error}"))?)
        };
        let secs = |name: &str, default: u64| -> Result<u64, Failure> {
            Ok(env::var(name)
                .map_or(Ok(default), |value| value.parse::<u64>())
                .map_err(|error| format!("{name}: {error}"))?)
        };
        let send = flag("HL_SEND");
        let trade_net = if flag("HL_MAINNET") { Network::Mainnet } else { Network::Testnet };
        let book = env::var("HL_BOOK").unwrap_or_else(|_| "bbo".to_owned());
        Ok(Config {
            coin: env::var("HL_COIN").unwrap_or_else(|_| "BTC".to_owned()),
            send,
            trade_net,
            // A dry run reads mainnet: testnet books are thin and slow, so their timing says little.
            data_net: if send { trade_net } else { Network::Mainnet },
            size: dec("HL_SIZE", "0")?,
            half_spread_bps: dec("HL_SPREAD_BPS", "5")?,
            behind_ticks: Dec::from_int(i64::try_from(secs("HL_BEHIND_TICKS", 1)?)?),
            skew_bps: dec("HL_SKEW_BPS", "5")?,
            max_position_usd: dec("HL_MAX_POSITION_USD", "50")?,
            max_loss_usd: dec("HL_MAX_LOSS_USD", "5")?,
            source: Source::parse(&book).ok_or("HL_BOOK is bbo or fast")?,
            min_requote: Duration::from_millis(secs("HL_MIN_REQUOTE_MS", 500)?),
            watchdog: Duration::from_secs(secs("HL_WATCHDOG_SECS", 10)?),
            report_every: Duration::from_secs(secs("HL_REPORT_SECS", 10)?.max(1)),
            run_for: env::var("HL_SECS").ok().map(|value| value.parse::<u64>().map(Duration::from_secs)).transpose()?,
            requote_after_switch: flag("HL_REQUOTE_AFTER_SWITCH"),
            spin: flag("HL_SPIN"),
        })
    }
}

/// A quote as the program believes it rests.
#[derive(Debug, Clone, Copy)]
struct Resting {
    cloid: Cloid,
    level: Level,
}

/// What a request was signed with: enough, with the action, to rebuild every stage of the hash.
#[derive(Debug, Clone, Copy)]
struct Signed {
    nonce: u64,
    signature: Signature,
}

/// What a request in flight changes, to read its answer against.
struct Pending {
    sent: Instant,
    /// Order and modify requests: the side of each order, in the order sent. Empty for a cancel.
    sides: Vec<Side>,
    /// The action and its signature, to explain a "does not exist" answer.
    action: Box<dyn L1Action>,
    signed: Signed,
    /// Whether this is a refresh of the dead man's switch rather than a requote.
    switch: bool,
}

/// Everything the loop keeps.
struct Maker {
    config: Config,
    account: Address,
    /// Sends actions only, so no read's wait or 429 holds them.
    trader: Client<Native, LocalKey>,
    /// Sends reads only.
    reader: Client<Native>,
    asset: Asset,
    grid: Grid,
    settings: Option<Settings>,
    resting: HashMap<Side, Resting>,
    pending: HashMap<u64, Pending>,
    inventory: Inventory,
    budget: Budget,
    dog: Watchdog,
    stats: Stats,
    last_requote: Option<Instant>,
    last_mid: Option<Dec>,
    switch_armed: bool,
    next_cloid: u128,
    raised_noted: bool,
    /// The last requote's plan, kept for its lists' memory.
    plan: Plan,
}

/// Why the loop ended.
enum Stop {
    Interrupted,
    TimeUp,
    LossLimit(Dec),
    FeedStopped,
    SwitchFired,
}

fn main() -> Result<(), Failure> {
    // The quoting loop runs on this thread; the feed's tasks run on the runtime's workers.
    let on_performance_cores = cores::prefer_performance_cores();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_start(|| {
            cores::prefer_performance_cores();
        })
        .build()?;
    runtime.block_on(start(on_performance_cores))
}

async fn start(on_performance_cores: bool) -> Result<(), Failure> {
    let config = Config::read()?;
    if cfg!(debug_assertions) {
        println!(
            "warning: this is a debug build, so the timing figures are not meaningful; run it with cargo run --release"
        );
    }
    let (key, throwaway) = signing_key(config.send)?;
    let account = match env::var("HL_ADDRESS") {
        Ok(text) => text.parse::<Address>().map_err(|error| format!("HL_ADDRESS: {error}"))?,
        Err(_) => key.address(),
    };
    let reader = Client::reader(Native::new(), config.trade_net, clock);

    println!(
        "{} on {}, {}",
        config.coin,
        net(config.trade_net),
        if config.send {
            "sending orders"
        } else {
            "dry run: reading the mainnet touch, signing for testnet, sending nothing"
        }
    );
    println!("book feed: {}; orders: {}", net(config.data_net), net(config.trade_net));
    if config.send && config.data_net != config.trade_net {
        return Err(format!(
            "the book feed ({}) and the orders ({}) are on different networks; quotes would follow another network's prices",
            net(config.data_net),
            net(config.trade_net)
        )
        .into());
    }
    if config.spin {
        println!("HL_SPIN=1: the quoting thread spins between frames and keeps one core at 100%");
    }
    if on_performance_cores {
        println!("quoting thread and runtime workers set to the user-interactive QoS class (performance cores)");
    }
    if throwaway {
        println!("no HL_PRIVATE_KEY: signing with a throwaway key made in memory ({})", key.address());
    }
    println!("account {account}, signer {}", key.address());
    let trader = Client::new(Native::new(), config.trade_net, clock, key);

    // The coin's asset id and grid on the network it trades on.
    let spent = || vec![reader.spent(), trader.spent()];
    let meta = reads::read("meta", spent, || perps::meta(&reader, Dex::Main)).await?.ok_or("no main dex")?;
    let (asset, perp) = meta
        .find(0, &config.coin)
        .ok_or_else(|| format!("{} is not listed on {}", config.coin, net(config.trade_net)))?;
    let grid = Grid::perp(perp.sz_decimals);
    println!("asset {}, {} size decimals, {} price places", asset.id(), grid.sz_decimals(), grid.price_places());

    // The request budget, read once; counted down locally after that.
    let at_start = match reads::read("rate limit", spent, || account::rate_limit(&reader, account)).await {
        Ok(limit) => {
            println!(
                "request budget: {} used of {}, {} left; {} USDC traded",
                limit.used,
                limit.cap,
                limit.remaining(),
                limit.volume
            );
            Some(limit.remaining())
        }
        Err(error) => {
            println!("request budget not read ({error}); pacing at the base gap only");
            None
        }
    };

    let mut maker = Maker {
        budget: Budget { at_start, used: 0, base_gap: config.min_requote },
        dog: Watchdog { limit: config.watchdog, last_book: Instant::now() },
        config,
        account,
        trader,
        reader,
        asset,
        grid,
        settings: None,
        resting: HashMap::new(),
        pending: HashMap::new(),
        inventory: Inventory::default(),
        stats: Stats::default(),
        last_requote: None,
        last_mid: None,
        switch_armed: false,
        next_cloid: u128::from(clock()) << 64,
        raised_noted: false,
        plan: Plan::default(),
    };

    let started = Instant::now();
    if maker.config.send {
        maker.arm_switch().await;
    }
    let ended = maker.run().await;
    if maker.config.send {
        maker.cancel_everything("exit").await;
        if maker.switch_armed {
            match maker.send_http(&ScheduleCancel::disarm()).await {
                Ok(_) => println!("dead man's switch disarmed"),
                Err(error) => println!("disarming the dead man's switch failed: {error}"),
            }
        }
    }
    println!("\n{}", maker.stats.report(started.elapsed(), maker.config.send));
    if let Some(mid) = maker.last_mid {
        let inventory = maker.inventory;
        println!(
            "position {} {}, {} fills, fees {}, pnl {} at {mid}",
            inventory.position,
            maker.config.coin,
            inventory.fills,
            inventory.fees,
            inventory.pnl(mid).unwrap_or(Dec::ZERO)
        );
    }
    match ended? {
        Stop::Interrupted => println!("stopped by Ctrl-C"),
        Stop::TimeUp => println!("stopped: HL_SECS reached"),
        Stop::LossLimit(pnl) => println!("stopped: the loss limit was reached (pnl {pnl})"),
        Stop::FeedStopped => println!("stopped: the feed stopped"),
        Stop::SwitchFired => println!(
            "stopped: the dead man's switch fired and the venue cancelled every order; set HL_REQUOTE_AFTER_SWITCH=1 to quote again instead"
        ),
    }
    Ok(())
}

impl Maker {
    async fn run(&mut self) -> Result<Stop, Failure> {
        let mut feed = Feed::start(Session::new(self.config.data_net));
        feed.subscribe(self.config.source.subscription(&self.config.coin)).await?;
        // Orders are sent on the data feed, so it only carries the account when it is the
        // network traded on.
        if self.config.send {
            feed.subscribe(Subscription::OrderUpdates { user: self.account }).await?;
            feed.subscribe(Subscription::UserFills { user: self.account, aggregate_by_time: false }).await?;
        }

        let mut report = tokio::time::interval(self.config.report_every);
        report.tick().await;
        let mut heartbeat = tokio::time::interval(SWITCH_EVERY);
        heartbeat.tick().await;
        let mut watch = tokio::time::interval(Duration::from_secs(1));
        let mut last_warm = Instant::now();
        let deadline = self.config.run_for.map(|run_for| tokio::time::Instant::now() + run_for);
        let started = Instant::now();
        self.dog.last_book = Instant::now();

        loop {
            tokio::select! {
                received = feed.next_with_text() => {
                    let arrived = Instant::now();
                    let Some((event, text)) = received else { return Ok(Stop::FeedStopped) };
                    match event {
                        Event::Frame(_) => {
                            let Some(text) = text else { continue };
                            // The frame read again here with the SDK's decoder, so the time it
                            // takes is measured on this thread.
                            let frame = live::decode(&text)?;
                            let parsed = arrived.elapsed();
                            if let Some(stop) = self.on_frame(&feed, frame, arrived, parsed).await? {
                                return Ok(stop);
                            }
                        }
                        Event::Opened => println!("connected to {}", net(self.config.data_net)),
                        Event::Closed { reason, retry } => {
                            println!("disconnected ({reason}); dialling again in {} ms, quotes rest until the watchdog or the switch acts", retry.after_ms);
                        }
                        Event::Unreadable(bad) => println!("unreadable frame: {bad}"),
                        _ => {}
                    }
                }
                _ = tokio::signal::ctrl_c() => return Ok(Stop::Interrupted),
                _ = sleep_until(deadline) => return Ok(Stop::TimeUp),
                _ = report.tick() => println!("{}", self.stats.report(started.elapsed(), self.config.send)),
                _ = heartbeat.tick(), if self.config.send && self.switch_armed => self.refresh_switch(&feed).await?,
                // With HL_SPIN=1 the loop never sleeps: this branch is always ready, so the thread
                // stays on its core at full clock, and once a millisecond it keeps the signing
                // path's code and tables in cache.
                () = tokio::task::yield_now(), if self.config.spin => {
                    if last_warm.elapsed() >= WARM_EVERY {
                        self.warm();
                        last_warm = Instant::now();
                    }
                }
                _ = watch.tick() => {
                    let oldest = self.pending.values().map(|pending| pending.sent).min();
                    if let Some(why) = self.dog.tripped(Instant::now(), oldest) {
                        println!("watchdog: {why} for {} s", self.config.watchdog.as_secs());
                        if self.config.send {
                            self.cancel_everything("watchdog").await;
                        }
                        self.resting.clear();
                        self.pending.clear();
                        self.dog.last_book = Instant::now();
                    }
                }
            }
        }
    }

    /// One frame: a touch requotes, an answer settles a request, fills move the inventory.
    async fn on_frame(
        &mut self,
        feed: &Feed,
        frame: Frame,
        arrived: Instant,
        parsed: Duration,
    ) -> Result<Option<Stop>, Failure> {
        if let Some((bid, ask)) = book::touch(&frame, &self.config.coin) {
            self.dog.last_book = arrived;
            self.stats.frames += 1;
            self.stats.parse.add(parsed);
            return self.on_touch(feed, bid, ask, arrived).await;
        }
        match frame {
            Frame::Post(reply) => {
                if let Some(pending) = self.pending.remove(&reply.id) {
                    if pending.switch {
                        match &reply.answer {
                            Answer::Action(ExchangeResponse::Ok(_)) => {}
                            Answer::Action(ExchangeResponse::Err(message)) | Answer::Error(message) => {
                                self.switch_refused(message);
                                self.explain_unknown(
                                    message,
                                    pending.action.as_ref(),
                                    pending.signed,
                                    self.websocket(),
                                );
                            }
                            _ => self.switch_refused("an answer of another kind"),
                        }
                        return Ok(None);
                    }
                    self.stats.ack.add(pending.sent.elapsed());
                    self.on_answer(&pending, &reply.answer);
                }
            }
            Frame::Error(refusal) => {
                if let Some(pending) = refusal.post_id().and_then(|id| self.pending.remove(&id)) {
                    if pending.switch {
                        self.switch_refused(&refusal.message);
                        return Ok(None);
                    }
                    self.stats.refused += 1;
                    println!("refused: {}", refusal.message);
                    self.explain_unknown(&refusal.message, pending.action.as_ref(), pending.signed, self.websocket());
                    for side in pending.sides {
                        self.resting.remove(&side);
                    }
                }
            }
            Frame::OrderUpdates(updates) => {
                let mut fired = false;
                for update in updates.iter().filter(|update| update.order.coin == self.config.coin) {
                    if update.status != "open" {
                        self.resting.retain(|_, resting| Some(resting.cloid) != update.order.cloid);
                    }
                    fired |= update.status == "scheduledCancel";
                }
                if fired {
                    // Every order is gone; nothing left to modify.
                    self.resting.clear();
                    if !self.config.requote_after_switch {
                        return Ok(Some(Stop::SwitchFired));
                    }
                    println!(
                        "the dead man's switch fired and the venue cancelled every order; placing new quotes (HL_REQUOTE_AFTER_SWITCH=1)"
                    );
                }
            }
            Frame::UserFills(fills) if !fills.is_snapshot => {
                for fill in fills.fills.iter().filter(|fill| fill.coin == self.config.coin) {
                    self.inventory
                        .fill(fill.side, fill.px, fill.sz, fill.fee)
                        .ok_or("the inventory left the decimal range")?;
                    println!(
                        "fill: {} {} at {}, position {}",
                        side_name(fill.side),
                        fill.sz,
                        fill.px,
                        self.inventory.position
                    );
                }
            }
            _ => {}
        }
        Ok(None)
    }

    /// A new touch: work out the quotes, and sign (and send) what changed.
    async fn on_touch(
        &mut self,
        feed: &Feed,
        best_bid: Dec,
        best_ask: Dec,
        arrived: Instant,
    ) -> Result<Option<Stop>, Failure> {
        let mid = best_bid
            .checked_add(best_ask)
            .and_then(|sum| sum.checked_div(Dec::from_int(2), 12, df_hyperliquid_core::utils::Round::Nearest));
        self.last_mid = mid;
        if let Some(mid) = mid
            && let Some(pnl) = self.inventory.pnl(mid)
            && pnl <= self.config.max_loss_usd.negate()
        {
            return Ok(Some(Stop::LossLimit(pnl)));
        }
        // One request at a time: the next requote waits for the last answer. A switch refresh in
        // flight does not hold it.
        if self.pending.values().any(|pending| !pending.switch) {
            return Ok(None);
        }
        if self.last_requote.is_some_and(|last| last.elapsed() < self.budget.gap()) {
            self.stats.paced += 1;
            return Ok(None);
        }

        // The settings are fixed at the first mid, before any timing, so the one-time printing
        // is not counted as quote logic.
        let settings = match self.settings {
            Some(settings) => settings,
            None => {
                let mid = mid.ok_or("no mid")?;
                let max_position = self
                    .config
                    .max_position_usd
                    .checked_div(mid, 12, df_hyperliquid_core::utils::Round::Down)
                    .ok_or("HL_MAX_POSITION_USD")?;
                let settings = Settings {
                    grid: self.grid,
                    half_spread_bps: self.config.half_spread_bps,
                    size: self.config.size,
                    skew_bps: self.config.skew_bps,
                    max_position,
                    behind_ticks: self.config.behind_ticks,
                };
                println!(
                    "quoting: half spread {} bps, {} ticks behind the best price, skew {} bps, max position {} {} (${} at {mid})",
                    settings.half_spread_bps,
                    settings.behind_ticks,
                    settings.skew_bps,
                    self.grid.size(max_position).unwrap_or(Dec::ZERO),
                    self.config.coin,
                    self.config.max_position_usd
                );
                self.settings = Some(settings);
                settings
            }
        };
        let quoted = Instant::now();
        let Some(want) = quote::quotes(&settings, best_bid, best_ask, self.inventory.position) else { return Ok(None) };
        // The plan's lists are kept between requotes, so working one out allocates nothing.
        let mut plan = std::mem::take(&mut self.plan);
        plan.clear();
        self.fill_plan(&want, &mut plan);
        self.stats.quote.add(quoted.elapsed());
        if want.raised_to_minimum && !self.raised_noted {
            println!(
                "a quote's size is raised to the venue's $10 minimum ({})",
                want.bid.or(want.ask).map_or(Dec::ZERO, |level| level.sz)
            );
            self.raised_noted = true;
        }
        if plan.is_empty() {
            self.stats.unchanged += 1;
            self.plan = plan;
            return Ok(None);
        }

        // Build, hash, sign and write each request body.
        let signing = Instant::now();
        let mut bodies = Vec::new();
        if !plan.modify.is_empty() {
            let action = BatchModify::new(
                plan.modify.iter().map(|(_, old, order)| Modify::new(OrderRef::Cloid(*old), *order)).collect(),
            );
            let (body, signed) = self.sign(&action)?;
            let sides = plan.modify.iter().map(|(side, _, _)| *side).collect::<Vec<_>>();
            bodies.push((body, signed, sides, Box::new(action) as Box<dyn L1Action>));
        }
        if !plan.place.is_empty() {
            let action = PlaceOrder::new(plan.place.iter().map(|(_, order)| *order).collect());
            let (body, signed) = self.sign(&action)?;
            bodies.push((body, signed, plan.place.iter().map(|(side, _)| *side).collect(), Box::new(action)));
        }
        if !plan.cancel.is_empty() {
            let action = CancelByCloid::new(plan.cancel.iter().map(|(_, cloid)| (self.asset, *cloid)).collect());
            let (body, signed) = self.sign(&action)?;
            bodies.push((body, signed, Vec::new(), Box::new(action)));
        }
        self.stats.sign.add(signing.elapsed());
        self.stats.total.add(arrived.elapsed());
        self.stats.requotes += 1;
        self.last_requote = Some(Instant::now());

        // What the quotes become once the requests are taken.
        for (side, _, order) in &plan.modify {
            self.resting.insert(
                *side,
                Resting {
                    cloid: order.cloid().ok_or("no cloid")?,
                    level: Level { px: order.price(), sz: order.size() },
                },
            );
        }
        for (side, order) in &plan.place {
            self.resting.insert(
                *side,
                Resting {
                    cloid: order.cloid().ok_or("no cloid")?,
                    level: Level { px: order.price(), sz: order.size() },
                },
            );
        }
        for (side, _) in &plan.cancel {
            self.resting.remove(side);
        }

        if self.config.send {
            let requests = u64::try_from(plan.modify.len() + plan.place.len() + plan.cancel.len()).unwrap_or(u64::MAX);
            self.budget.used = self.budget.used.saturating_add(requests);
            for (body, signed, sides, action) in bodies {
                let sent = Instant::now();
                let id = feed.post(Post::action(&body)?).await?;
                self.pending.insert(id, Pending { sent, sides, action, signed, switch: false });
            }
        }
        self.plan = plan;
        Ok(None)
    }

    /// What has to change for the quotes to become `want`.
    fn fill_plan(&mut self, want: &Quotes, plan: &mut Plan) {
        for side in [Side::Buy, Side::Sell] {
            match (self.resting.get(&side).copied(), want.on(side)) {
                (Some(now), Some(level)) if quote::needs_change(self.grid, now.level, level) => {
                    let order = self.order(side, level);
                    plan.modify.push((side, now.cloid, order));
                }
                (None, Some(level)) => {
                    let order = self.order(side, level);
                    plan.place.push((side, order));
                }
                (Some(now), None) => plan.cancel.push((side, now.cloid)),
                _ => {}
            }
        }
    }

    /// A post-only limit order with a client id of its own.
    fn order(&mut self, side: Side, level: Level) -> Order {
        self.next_cloid += 1;
        Order::limit(self.asset, side, level.px, level.sz, Tif::Alo).with_cloid(Cloid::from_u128(self.next_cloid))
    }

    /// The body `/exchange` takes for `action`, signed for the network traded on, and what it
    /// was signed with.
    fn sign(&self, action: &dyn L1Action) -> Result<(String, Signed), Failure> {
        let nonce = self.trader.next_nonce();
        let signature: Signature =
            self.trader.signer().sign_digest(&action.digest(nonce, None, self.config.trade_net))?;
        Ok((action.body(nonce, &signature, None), Signed { nonce, signature }))
    }

    /// Builds, hashes and signs a throwaway action and writes its body, so the next requote finds
    /// that code and its tables in cache. Nothing is sent and no nonce is used.
    fn warm(&self) {
        let action = ScheduleCancel::at(1);
        let digest = action.digest(1, None, self.config.trade_net);
        if let Ok(signature) = self.trader.signer().sign_digest(&digest) {
            std::hint::black_box(action.body(1, &signature, None));
        }
    }

    /// Signs `action` and posts it over HTTP once, through the client that sends no reads.
    async fn send_http(&self, action: &dyn L1Action) -> Result<Accepted, Failure> {
        let (body, signed) = self.sign(action)?;
        match exchange(&self.trader, body).await {
            Ok(accepted) => Ok(accepted),
            Err(ApiError::Refused(message)) => {
                let sent_to = format!("{}/exchange", self.config.trade_net.api_url());
                self.explain_unknown(&message, action, signed, sent_to);
                Err(message.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// On "User or API Wallet ... does not exist", prints the likely causes, every input of the
    /// hash and each stage of the signature. Prints nothing for any other answer.
    fn explain_unknown(&self, message: &str, action: &dyn L1Action, signed: Signed, sent_to: String) {
        if !refusal::does_not_exist(message) {
            return;
        }
        let breakdown = Breakdown::of(action, signed.nonce, None, self.config.trade_net);
        let recovered = breakdown.recover(&signed.signature).map_err(|error| error.to_string());
        let context = refusal::Context {
            network: self.config.trade_net,
            sent_to,
            nonce: signed.nonce,
            vault: None,
            expires_after: None,
            key_address: self.trader.signer().address(),
            account: self.account,
        };
        println!("{}", refusal::explain(message, &breakdown, recovered, &context));
    }

    /// Where requotes are posted: the websocket of the network read, which is the network traded
    /// on whenever orders are sent.
    fn websocket(&self) -> String {
        format!("{} (websocket post)", self.config.data_net.ws_url())
    }

    /// Reads an answer: a resting quote stays, a refused or filled one is forgotten.
    fn on_answer(&mut self, pending: &Pending, answer: &Answer) {
        let refusal = match answer {
            Answer::Action(ExchangeResponse::Ok(accepted)) if !pending.sides.is_empty() => {
                match orders(accepted, pending.sides.len()) {
                    Ok(answers) => {
                        for (side, answer) in pending.sides.iter().zip(answers) {
                            match answer {
                                OrderAnswer::Resting { .. } => {}
                                OrderAnswer::Refused(message) => {
                                    self.count_refusal(&message);
                                    self.explain_unknown(
                                        &message,
                                        pending.action.as_ref(),
                                        pending.signed,
                                        self.websocket(),
                                    );
                                    self.resting.remove(side);
                                }
                                _ => {
                                    self.resting.remove(side);
                                }
                            }
                        }
                        None
                    }
                    Err(error) => Some(error.to_string()),
                }
            }
            Answer::Action(ExchangeResponse::Ok(_)) => None,
            Answer::Action(ExchangeResponse::Err(message)) | Answer::Error(message) => Some(message.clone()),
            _ => Some("an answer of another kind".to_owned()),
        };
        if let Some(message) = refusal {
            self.count_refusal(&message);
            self.explain_unknown(&message, pending.action.as_ref(), pending.signed, self.websocket());
            for side in &pending.sides {
                self.resting.remove(side);
            }
        }
    }

    fn count_refusal(&mut self, message: &str) {
        if WOULD_CROSS.iter().any(|words| message.contains(words)) {
            self.stats.would_cross += 1;
        } else {
            self.stats.refused += 1;
            println!("refused: {message}");
        }
    }

    /// Tries the dead man's switch once, at the start, over HTTP; says why when the venue refuses.
    async fn arm_switch(&mut self) {
        let at = clock().saturating_add(SWITCH_AHEAD_MS);
        self.budget.used = self.budget.used.saturating_add(1);
        match self.send_http(&ScheduleCancel::at(at)).await {
            Ok(_) => {
                println!(
                    "dead man's switch armed: the venue cancels every open order if this program stops renewing it"
                );
                self.switch_armed = true;
            }
            Err(error) => self.switch_refused(&error.to_string()),
        }
    }

    /// Refreshes an accepted switch over the websocket, so the quoting loop does not wait for it.
    async fn refresh_switch(&mut self, feed: &Feed) -> Result<(), Failure> {
        let action = ScheduleCancel::at(clock().saturating_add(SWITCH_AHEAD_MS));
        let (body, signed) = self.sign(&action)?;
        self.budget.used = self.budget.used.saturating_add(1);
        let sent = Instant::now();
        let id = feed.post(Post::action(&body)?).await?;
        self.pending.insert(id, Pending { sent, sides: Vec::new(), action: Box::new(action), signed, switch: true });
        Ok(())
    }

    /// The venue refused the switch, at the start or on a refresh: it is not sent again.
    fn switch_refused(&mut self, message: &str) {
        println!(
            "scheduleCancel refused for this account; orders are cancelled on exit only. The venue said: {message}"
        );
        self.switch_armed = false;
    }

    /// Cancels the quotes this program knows of, then any other open order on the coin.
    async fn cancel_everything(&mut self, why: &str) {
        let known: Vec<(Asset, Cloid)> = self.resting.values().map(|resting| (self.asset, resting.cloid)).collect();
        if !known.is_empty() {
            match self.send_http(&CancelByCloid::new(known)).await {
                Ok(_) => println!("{why}: cancelled the quotes"),
                Err(error) => println!("{why}: cancelling the quotes failed: {error}"),
            }
        }
        self.resting.clear();
        let spent = || vec![self.reader.spent(), self.trader.spent()];
        let open = reads::read("open orders", spent, || {
            account::history::orders::open_orders(&self.reader, self.account, Dex::Main)
        })
        .await;
        match open {
            Ok(open) => {
                let left: Vec<(Asset, u64)> = open
                    .iter()
                    .filter(|order| order.coin == self.config.coin)
                    .map(|order| (self.asset, order.oid))
                    .collect();
                if !left.is_empty() {
                    match self.send_http(&Cancel::new(left)).await {
                        Ok(_) => println!("{why}: cancelled the other open orders on {}", self.config.coin),
                        Err(error) => println!("{why}: cancelling the other open orders failed: {error}"),
                    }
                }
            }
            Err(error) => print_unread(why, &error),
        }
    }
}

/// The requests one requote needs.
#[derive(Debug, Default)]
struct Plan {
    /// Each quote that moves: its side, the cloid it replaces, the new order.
    modify: Vec<(Side, Cloid, Order)>,
    /// Each side with nothing resting.
    place: Vec<(Side, Order)>,
    /// Each side no longer quoted: its side and cloid.
    cancel: Vec<(Side, Cloid)>,
}

impl Plan {
    fn clear(&mut self) {
        self.modify.clear();
        self.place.clear();
        self.cancel.clear();
    }

    fn is_empty(&self) -> bool {
        self.modify.is_empty() && self.place.is_empty() && self.cancel.is_empty()
    }
}

fn print_unread(why: &str, error: &ApiError) {
    println!("{why}: the open orders were not read ({error}); check them on the venue");
}

/// The signing key: `HL_PRIVATE_KEY`, or for a dry run a throwaway one made in memory.
/// `HL_PRIVATE_KEY_FILE` names a file that holds the key as hex, so the key stays off the
/// command line and out of the shell's history.
fn signing_key(send: bool) -> Result<(LocalKey, bool), Failure> {
    let secret = match (env::var("HL_PRIVATE_KEY"), env::var("HL_PRIVATE_KEY_FILE")) {
        (Ok(_), Ok(_)) => return Err("set HL_PRIVATE_KEY or HL_PRIVATE_KEY_FILE, not both".into()),
        (Ok(secret), Err(_)) => Some(Zeroizing::new(secret)),
        (Err(_), Ok(path)) => Some(Zeroizing::new(
            std::fs::read_to_string(&path).map_err(|error| format!("HL_PRIVATE_KEY_FILE {path}: {error}"))?,
        )),
        (Err(_), Err(_)) => None,
    };
    match secret {
        Some(secret) => Ok((LocalKey::from_hex(secret.trim())?, false)),
        None if send => Err("HL_SEND=1 needs HL_PRIVATE_KEY_FILE or HL_PRIVATE_KEY".into()),
        None => {
            let mut secret = Zeroizing::new([0_u8; 32]);
            getrandom02::getrandom(secret.as_mut())
                .map_err(|error| format!("no randomness for a throwaway key: {error}"))?;
            Ok((LocalKey::from_bytes(&secret)?, true))
        }
    }
}

async fn sleep_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

fn net(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        _ => "testnet",
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Buy => "bought",
        Side::Sell => "sold",
    }
}
