"""Benchmarks of the official Hyperliquid Python SDK (hyperliquid-python-sdk).

The same subcommands and JSON output as the Rust binaries in bench/:

    sign-check                  the signature of the fixed order, for the cross-SDK check
    sign                        hash and sign the fixed order; build, sign and serialize it
    parse --corpus <path>       decode the recorded websocket frames as the SDK does
    serve --px <p> --sz <s>     answer `info` and `exchange` lines on stdin, one JSON line each
    live --secs <n>             BTC l2Book and trades from mainnet for n seconds

Each call is the one the SDK's README and examples use. The ECDSA backend of eth-keys is
chosen with ECC_BACKEND_CLASS; run.py sets it for every run.
"""

import gzip
import json
import os
import sys
import threading
import time

import eth_account
from eth_keys.backends import get_backend_class
from hyperliquid.exchange import Exchange
from hyperliquid.info import Info
from hyperliquid.utils import constants
from hyperliquid.utils.signing import (
    order_request_to_order_wire,
    order_wires_to_order_action,
    sign_l1_action,
)

FIXED_KEY = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
FIXED_NONCE = 1_700_000_000_000
FIXED_ASSET = 0
FIXED_PX = 50000.0
FIXED_SZ = 0.001

# A quick run (BENCH_QUICK=1) checks that everything runs rather than measuring.
QUICK = os.environ.get("BENCH_QUICK") == "1"
SIGN_WARMUP = 20 if QUICK else 200
PARSE_WARMUP_PASSES = 1 if QUICK else 3
PARSE_PASSES = 5 if QUICK else 20


def flag(name, default=None):
    args = sys.argv[2:]
    if name in args:
        return args[args.index(name) + 1]
    return default


def backend():
    return get_backend_class().__name__


def percentile(sorted_samples, fraction):
    if not sorted_samples:
        return 0
    rank = round((len(sorted_samples) - 1) * fraction)
    return sorted_samples[min(rank, len(sorted_samples) - 1)]


def summary_ns(samples):
    ordered = sorted(samples)
    mean = sum(ordered) / len(ordered) if ordered else 0.0
    return {
        "n": len(ordered),
        "min_ns": ordered[0] if ordered else 0,
        "median_ns": percentile(ordered, 0.5),
        "p90_ns": percentile(ordered, 0.9),
        "p99_ns": percentile(ordered, 0.99),
        "max_ns": ordered[-1] if ordered else 0,
        "mean_ns": mean,
        "ops_per_s": 1e9 / mean if mean else 0.0,
    }


def time_each(count, run):
    samples = []
    clock = time.perf_counter_ns
    for _ in range(count):
        start = clock()
        run()
        samples.append(clock() - start)
    return samples


def fixed_request(px=FIXED_PX, sz=FIXED_SZ):
    return {
        "coin": "BTC",
        "is_buy": True,
        "sz": sz,
        "limit_px": px,
        "order_type": {"limit": {"tif": "Alo"}},
        "reduce_only": False,
    }


def hex65(signature):
    return "0x%064x%064x%02x" % (int(signature["r"], 16), int(signature["s"], 16), signature["v"])


def sign_check():
    wallet = eth_account.Account.from_key(FIXED_KEY)
    action = order_wires_to_order_action([order_request_to_order_wire(fixed_request(), FIXED_ASSET)])
    signature = sign_l1_action(wallet, action, None, FIXED_NONCE, None, True)
    return {
        "sdk": "hyperliquid-python-sdk",
        "backend": backend(),
        "address": wallet.address.lower(),
        "signature": hex65(signature),
        "action": action,
    }


def sign():
    samples = int(flag("--samples", "2000"))
    wallet = eth_account.Account.from_key(FIXED_KEY)
    action = order_wires_to_order_action([order_request_to_order_wire(fixed_request(), FIXED_ASSET)])

    # Hash and sign an order action that is already built: msgpack, keccak, EIP-712, ECDSA.
    def hash_and_sign():
        return sign_l1_action(wallet, action, None, FIXED_NONCE, None, True)

    # From float price and size to the JSON body Exchange._post_action posts.
    def build_sign_body():
        wire = order_request_to_order_wire(fixed_request(), FIXED_ASSET)
        built = order_wires_to_order_action([wire])
        signature = sign_l1_action(wallet, built, None, FIXED_NONCE, None, True)
        payload = {
            "action": built,
            "nonce": FIXED_NONCE,
            "signature": signature,
            "vaultAddress": None,
            "expiresAfter": None,
        }
        return json.dumps(payload)

    for _ in range(SIGN_WARMUP):
        hash_and_sign()
    signed = time_each(samples, hash_and_sign)
    for _ in range(SIGN_WARMUP):
        build_sign_body()
    bodies = time_each(samples, build_sign_body)
    return {
        "sdk": "hyperliquid-python-sdk",
        "backend": backend(),
        "sign": summary_ns(signed),
        "build_sign_body": summary_ns(bodies),
    }


def load_corpus(path):
    frames = []
    with gzip.open(path, "rt", encoding="utf-8") as lines:
        for line in lines:
            line = line.rstrip("\n")
            if line:
                frames.append((json.loads(line).get("channel", ""), line))
    return frames


def throughput(frames, decode):
    texts = [text for _, text in frames]
    size = sum(len(text.encode("utf-8")) for text in texts)
    failed = 0
    for text in texts:
        try:
            decode(text)
        except Exception:  # noqa: BLE001 - a frame that does not read is counted, not fatal
            failed += 1
    for _ in range(PARSE_WARMUP_PASSES):
        for text in texts:
            decode(text)
    passes = []
    for _ in range(PARSE_PASSES):
        start = time.perf_counter_ns()
        for text in texts:
            decode(text)
        passes.append(time.perf_counter_ns() - start)
    seconds = sum(passes) / 1e9
    frame_ns = []
    for text in texts:
        start = time.perf_counter_ns()
        decode(text)
        frame_ns.append(time.perf_counter_ns() - start)
    frame_ns.sort()
    return {
        "frames": len(texts),
        "bytes": size,
        "failed": failed,
        "passes": PARSE_PASSES,
        "frames_per_s": len(texts) * PARSE_PASSES / seconds,
        "mb_per_s": size * PARSE_PASSES / seconds / 1e6,
        "median_pass_ns": percentile(sorted(passes), 0.5),
        "ns_per_frame": seconds * 1e9 / (len(texts) * PARSE_PASSES),
        "frame_median_ns": percentile(frame_ns, 0.5),
        "frame_p99_ns": percentile(frame_ns, 0.99),
        "allocations_per_frame": None,
    }


def parse():
    frames = load_corpus(flag("--corpus"))

    # What WebsocketManager.on_message does before it calls the subscriber: json.loads, then
    # the identifier that routes the message. The subscriber gets a dict of strings.
    from hyperliquid.websocket_manager import ws_msg_to_identifier

    def decode(text):
        message = json.loads(text)
        ws_msg_to_identifier(message)
        return message

    return {
        "sdk": "hyperliquid-python-sdk",
        "parse": {
            "all": throughput(frames, decode),
            "l2Book": throughput([f for f in frames if f[0] == "l2Book"], decode),
            "trades": throughput([f for f in frames if f[0] == "trades"], decode),
        },
    }


def serve():
    px = float(flag("--px"))
    sz = float(flag("--sz"))
    # A fresh random key that has never held anything: the venue has no account for it.
    wallet = eth_account.Account.create()
    info = Info(constants.MAINNET_API_URL, skip_ws=True)
    exchange = Exchange(wallet, constants.MAINNET_API_URL)
    for line in sys.stdin:
        command = line.strip()
        if command in ("quit", ""):
            break
        if command == "info":
            start = time.perf_counter_ns()
            try:
                book = info.l2_snapshot("BTC")
                ns = time.perf_counter_ns() - start
                answer = {"ok": True, "ns": ns, "answer": f"{len(book['levels'][0])} bids {len(book['levels'][1])} asks"}
            except Exception as error:  # noqa: BLE001
                answer = {"ok": False, "ns": time.perf_counter_ns() - start, "answer": str(error)}
        elif command == "exchange":
            start = time.perf_counter_ns()
            try:
                result = exchange.order("BTC", True, sz, px, {"limit": {"tif": "Alo"}})
                ns = time.perf_counter_ns() - start
                # The venue refuses the order of an account that does not exist; that answer,
                # read into a dict, is the expected one.
                answer = {"ok": isinstance(result, dict) and "status" in result, "ns": ns, "answer": json.dumps(result)}
            except Exception as error:  # noqa: BLE001
                answer = {"ok": False, "ns": time.perf_counter_ns() - start, "answer": str(error)}
        elif command == "address":
            answer = {"ok": True, "ns": 0, "answer": wallet.address.lower()}
        else:
            answer = {"ok": False, "ns": 0, "answer": f"unknown command {command}"}
        sys.stdout.write(json.dumps(answer) + "\n")
        sys.stdout.flush()


def live():
    secs = int(flag("--secs"))
    lock = threading.Lock()
    tally = {"book_frames": 0, "bbo_frames": 0, "trade_frames": 0, "trades": 0, "age_ms": []}

    def now_ms():
        return int(time.time() * 1000)

    def on_book(message):
        with lock:
            tally["book_frames"] += 1
            tally["age_ms"].append(max(0, now_ms() - message["data"]["time"]))

    def on_bbo(message):
        with lock:
            tally["bbo_frames"] += 1
            tally["age_ms"].append(max(0, now_ms() - message["data"]["time"]))

    def on_trades(message):
        with lock:
            tally["trade_frames"] += 1
            tally["trades"] += len(message["data"])
            newest = max(trade["time"] for trade in message["data"])
            tally["age_ms"].append(max(0, now_ms() - newest))

    start = time.monotonic()
    info = Info(constants.MAINNET_API_URL, skip_ws=False)
    info.subscribe({"type": "l2Book", "coin": "BTC", "fast": True}, on_book)
    info.subscribe({"type": "bbo", "coin": "BTC"}, on_bbo)
    info.subscribe({"type": "trades", "coin": "BTC"}, on_trades)
    time.sleep(max(0.0, secs - (time.monotonic() - start)))
    with lock:
        ages = sorted(tally["age_ms"])
        report = {
            "seconds": secs,
            "book_frames": tally["book_frames"],
            "bbo_frames": tally["bbo_frames"],
            "trade_frames": tally["trade_frames"],
            "trades": tally["trades"],
            "other_frames": None,
            "unreadable": None,
            "connects": 1,
            "age_ms_median": percentile(ages, 0.5),
            "age_ms_p99": percentile(ages, 0.99),
            "age_ms": list(tally["age_ms"]),
        }
    info.disconnect_websocket()
    return {"sdk": "hyperliquid-python-sdk", "live": report}


def prefer_performance_cores():
    """Asks macOS to run this thread at the user-interactive quality of service, as the Rust
    binaries do, so the scheduler keeps it on a performance core. The timed signing and parsing
    run on this thread. Does nothing elsewhere."""
    if sys.platform != "darwin":
        return
    import ctypes

    user_interactive = 0x21  # QOS_CLASS_USER_INTERACTIVE in <pthread/qos.h>
    ctypes.CDLL("/usr/lib/libSystem.B.dylib").pthread_set_qos_class_self_np(user_interactive, 0)


def main():
    prefer_performance_cores()
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "serve":
        serve()
        return
    commands = {"sign-check": sign_check, "sign": sign, "parse": parse, "live": live}
    if command not in commands:
        sys.exit("usage: bench_py.py sign-check | sign | parse | serve | live")
    print(json.dumps(commands[command]()))
    sys.stdout.flush()
    os._exit(0)


if __name__ == "__main__":
    main()
