// Benchmarks of nktkas's TypeScript SDK, @nktkas/hyperliquid, on Node.
//
// The same subcommands and JSON output as the Rust binaries in bench/:
//
//   sign-check                  the signature of the fixed order, for the cross-SDK check
//   sign                        hash and sign the fixed order; build, sign and serialize it
//   parse --corpus <path>       decode the recorded websocket frames as the SDK does
//   serve --px <p> --sz <s>     answer `info` and `exchange` lines on stdin, one JSON line each
//   live --secs <n>             BTC l2Book (fast), bbo and trades from mainnet for n seconds
//
// Signing uses the calls the SDK's signing documentation shows: `canonicalize` with the order
// schema, `signL1Action` with a viem account, and `JSON.stringify` of the body.

import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { gunzipSync } from "node:zlib";

import { ExchangeClient, HttpTransport, InfoClient, SubscriptionClient, WebSocketTransport } from "@nktkas/hyperliquid";
import { OrderRequest } from "@nktkas/hyperliquid/api/exchange";
import { canonicalize, signL1Action } from "@nktkas/hyperliquid/signing";
import { Agent } from "undici";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";

const SDK = "@nktkas/hyperliquid";
const FIXED_KEY = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";
const FIXED_NONCE = 1_700_000_000_000;
const FIXED_ASSET = 0;
const FIXED_PX = "50000";
const FIXED_SZ = "0.001";

// A quick run (BENCH_QUICK=1) checks that everything runs rather than measuring.
const QUICK = process.env.BENCH_QUICK === "1";
const SIGN_WARMUP = QUICK ? 200 : 2_000;
const SIGN_SAMPLES = QUICK ? 2_000 : 20_000;
const PARSE_WARMUP_PASSES = QUICK ? 1 : 3;
const PARSE_PASSES = QUICK ? 5 : 20;

function flag(name) {
  const args = process.argv.slice(3);
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : undefined;
}

function percentile(sorted, fraction) {
  if (sorted.length === 0) return 0;
  const rank = Math.round((sorted.length - 1) * fraction);
  return sorted[Math.min(rank, sorted.length - 1)];
}

function summaryNs(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = sorted.length ? sorted.reduce((sum, value) => sum + value, 0) / sorted.length : 0;
  return {
    n: sorted.length,
    min_ns: sorted[0] ?? 0,
    median_ns: percentile(sorted, 0.5),
    p90_ns: percentile(sorted, 0.9),
    p99_ns: percentile(sorted, 0.99),
    max_ns: sorted[sorted.length - 1] ?? 0,
    mean_ns: mean,
    ops_per_s: mean > 0 ? 1e9 / mean : 0,
  };
}

const clock = () => Number(process.hrtime.bigint());

async function timeEach(count, run) {
  const samples = new Array(count);
  for (let at = 0; at < count; at++) {
    const start = clock();
    await run();
    samples[at] = clock() - start;
  }
  return samples;
}

async function warm(count, run) {
  for (let at = 0; at < count; at++) await run();
}

// The order every SDK signs: a post-only BTC buy, in the wire form the order schema defines.
function orderAction(px, sz) {
  return canonicalize(OrderRequest.entries.action, {
    type: "order",
    orders: [{ a: FIXED_ASSET, b: true, p: px, s: sz, r: false, t: { limit: { tif: "Alo" } } }],
    grouping: "na",
  });
}

function hex65(signature) {
  const pad = (hex) => hex.slice(2).padStart(64, "0");
  return `0x${pad(signature.r)}${pad(signature.s)}${signature.v.toString(16).padStart(2, "0")}`;
}

async function signCheck() {
  const wallet = privateKeyToAccount(FIXED_KEY);
  const action = orderAction(FIXED_PX, FIXED_SZ);
  const signature = await signL1Action({ wallet, action, nonce: FIXED_NONCE });
  return { sdk: SDK, address: wallet.address.toLowerCase(), signature: hex65(signature), action };
}

async function sign() {
  const wallet = privateKeyToAccount(FIXED_KEY);
  const action = orderAction(FIXED_PX, FIXED_SZ);

  // Hash and sign an order action that is already built.
  const hashAndSign = () => signL1Action({ wallet, action, nonce: FIXED_NONCE });
  await warm(SIGN_WARMUP, hashAndSign);
  const signed = await timeEach(SIGN_SAMPLES, hashAndSign);

  // From price and size strings to the JSON body `/exchange` takes.
  const buildSignBody = async () => {
    const built = orderAction(FIXED_PX, FIXED_SZ);
    const signature = await signL1Action({ wallet, action: built, nonce: FIXED_NONCE });
    return JSON.stringify({ action: built, signature, nonce: FIXED_NONCE });
  };
  await warm(SIGN_WARMUP, buildSignBody);
  const bodies = await timeEach(SIGN_SAMPLES, buildSignBody);

  return { sdk: SDK, sign: summaryNs(signed), build_sign_body: summaryNs(bodies) };
}

function loadCorpus(path) {
  return gunzipSync(readFileSync(path))
    .toString("utf8")
    .split("\n")
    .filter((line) => line.length > 0)
    .map((text) => ({ channel: JSON.parse(text).channel ?? "", text }));
}

function throughput(frames, decode) {
  const texts = frames.map((frame) => frame.text);
  const bytes = texts.reduce((sum, text) => sum + Buffer.byteLength(text), 0);
  const failed = texts.filter((text) => !decode(text)).length;
  for (let pass = 0; pass < PARSE_WARMUP_PASSES; pass++) for (const text of texts) decode(text);
  const passes = [];
  for (let pass = 0; pass < PARSE_PASSES; pass++) {
    const start = clock();
    for (const text of texts) decode(text);
    passes.push(clock() - start);
  }
  const seconds = passes.reduce((sum, ns) => sum + ns, 0) / 1e9;
  const frameNs = texts.map((text) => {
    const start = clock();
    decode(text);
    return clock() - start;
  });
  frameNs.sort((a, b) => a - b);
  return {
    frames: texts.length,
    bytes,
    failed,
    passes: PARSE_PASSES,
    frames_per_s: (texts.length * PARSE_PASSES) / seconds,
    mb_per_s: (bytes * PARSE_PASSES) / seconds / 1e6,
    median_pass_ns: percentile([...passes].sort((a, b) => a - b), 0.5),
    ns_per_frame: (seconds * 1e9) / (texts.length * PARSE_PASSES),
    frame_median_ns: percentile(frameNs, 0.5),
    frame_p99_ns: percentile(frameNs, 0.99),
    allocations_per_frame: null,
  };
}

async function parse() {
  const frames = loadCorpus(flag("--corpus"));
  // The SDK's own frame reader: WebSocketTransport hands each `message` event of its socket to a
  // HyperliquidEventTarget, which parses the text with JSON.parse and dispatches a CustomEvent
  // named after the channel. The class is not in the package's exports, so it is loaded from its
  // file. One reused `message` event stands in for the socket's, so the runtime's own cost of
  // making that event is not counted.
  const events = await import(new URL("./node_modules/@nktkas/hyperliquid/esm/transport/websocket/_events.js", import.meta.url));
  const socket = new EventTarget();
  const target = new events.HyperliquidEventTarget(socket);
  let delivered = 0;
  for (const channel of ["l2Book", "trades"]) target.addEventListener(channel, () => delivered++);
  const message = new Event("message");
  const decode = (text) => {
    const before = delivered;
    message.data = text;
    socket.dispatchEvent(message);
    return delivered > before;
  };
  return {
    sdk: SDK,
    parse: {
      all: throughput(frames, decode),
      l2Book: throughput(frames.filter((frame) => frame.channel === "l2Book"), decode),
      trades: throughput(frames.filter((frame) => frame.channel === "trades"), decode),
    },
  };
}

async function serve() {
  const px = flag("--px");
  const sz = flag("--sz");
  // A fresh random key that has never held anything: the venue has no account for it.
  const wallet = privateKeyToAccount(generatePrivateKey());
  // Node's fetch closes a connection after 4 s idle when the server sends no keep-alive timeout,
  // and api.hyperliquid.xyz sends none. With every SDK taking its turn, each one's requests are
  // about 5 s apart, so with the default every request here would open a new TLS connection,
  // while the Rust, Go and Python clients keep an idle connection for 90 s or more. This agent
  // keeps it for 90 s, passed through the transport's `fetchOptions`. undici is pinned to the
  // version Node 22.14 bundles.
  const dispatcher = new Agent({ keepAliveTimeout: 90_000 });
  const transport = new HttpTransport({ fetchOptions: { dispatcher } });
  const info = new InfoClient({ transport });
  const exchange = new ExchangeClient({ transport, wallet });
  const lines = createInterface({ input: process.stdin });
  for await (const line of lines) {
    const command = line.trim();
    if (command === "quit" || command === "") break;
    let answer;
    const start = clock();
    if (command === "info") {
      try {
        const book = await info.l2Book({ coin: "BTC" });
        answer = { ok: true, ns: clock() - start, answer: `${book.levels[0].length} bids ${book.levels[1].length} asks` };
      } catch (error) {
        answer = { ok: false, ns: clock() - start, answer: String(error) };
      }
    } else if (command === "exchange") {
      try {
        const result = await exchange.order({
          orders: [{ a: FIXED_ASSET, b: true, p: px, s: sz, r: false, t: { limit: { tif: "Alo" } } }],
          grouping: "na",
        });
        answer = { ok: true, ns: clock() - start, answer: `accepted: ${JSON.stringify(result)}` };
      } catch (error) {
        // The SDK throws the venue's refusal as an ApiRequestError; that refusal is the expected answer.
        const ns = clock() - start;
        const text = String(error?.message ?? error);
        answer = { ok: text.includes("does not exist") || text.includes("User or API Wallet"), ns, answer: text };
      }
    } else if (command === "address") {
      answer = { ok: true, ns: 0, answer: wallet.address.toLowerCase() };
    } else {
      answer = { ok: false, ns: 0, answer: `unknown command ${command}` };
    }
    process.stdout.write(`${JSON.stringify(answer)}\n`);
  }
}

async function live() {
  const secs = Number(flag("--secs"));
  const tally = { book_frames: 0, bbo_frames: 0, trade_frames: 0, trades: 0, connects: 0, age_ms: [] };
  const aged = (venueMs) => tally.age_ms.push(Math.max(0, Date.now() - venueMs));
  const deadline = Date.now() + secs * 1000;
  const transport = new WebSocketTransport();
  transport.socket.addEventListener("open", () => tally.connects++);
  const client = new SubscriptionClient({ transport });
  await client.l2Book({ coin: "BTC", fast: true }, (book) => {
    tally.book_frames++;
    aged(book.time);
  });
  await client.bbo({ coin: "BTC" }, (bbo) => {
    tally.bbo_frames++;
    aged(bbo.time);
  });
  await client.trades({ coin: "BTC" }, (trades) => {
    tally.trade_frames++;
    tally.trades += trades.length;
    if (trades.length) aged(Math.max(...trades.map((trade) => trade.time)));
  });
  await new Promise((resolve) => setTimeout(resolve, Math.max(0, deadline - Date.now())));
  const ages = [...tally.age_ms].sort((a, b) => a - b);
  const report = {
    seconds: secs,
    book_frames: tally.book_frames,
    bbo_frames: tally.bbo_frames,
    trade_frames: tally.trade_frames,
    trades: tally.trades,
    other_frames: null,
    // The SDK drops a frame that is not JSON without a trace, so they cannot be counted.
    unreadable: null,
    connects: tally.connects,
    age_ms_median: percentile(ages, 0.5),
    age_ms_p99: percentile(ages, 0.99),
    age_ms: tally.age_ms,
  };
  transport.close();
  return { sdk: SDK, live: report };
}

const commands = { "sign-check": signCheck, sign, parse, live };
const command = process.argv[2] ?? "";
if (command === "serve") {
  await serve();
  process.exit(0);
}
if (!(command in commands)) {
  console.error("usage: node bench.mjs sign-check | sign | parse | serve | live");
  process.exit(1);
}
console.log(JSON.stringify(await commands[command]()));
process.exit(0);
