"""Records mainnet websocket frames for the offline parse benchmark.

Subscribes to trades for a few busy perps and to l2Book for the most traded perps (a single
book changes only every few seconds, so many books are needed for a few thousand frames),
keeps each text frame exactly as it arrived, and stops once it has --per-channel frames of each channel (or after --max-secs).
Writes one frame a line, in arrival order, gzipped:

    uv run python record.py --out ../data/frames.jsonl.gz --per-channel 2000 --books 150

It uses websocket-client directly, not any of the SDKs under test, so the corpus is the same
bytes for all of them.
"""

import gzip
import json
import sys
import threading
import time
import urllib.request

import websocket

URL = "wss://api.hyperliquid.xyz/ws"
TRADE_COINS = ["BTC", "ETH", "SOL", "HYPE", "XRP", "DOGE"]
CHANNELS = ("l2Book", "trades")


def flag(name, default):
    args = sys.argv[1:]
    return args[args.index(name) + 1] if name in args else default


def busiest_perps(count):
    """The `count` main-dex perps with the most notional volume over the last day."""
    request = urllib.request.Request(
        "https://api.hyperliquid.xyz/info",
        data=json.dumps({"type": "metaAndAssetCtxs"}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=10) as answer:
        meta, ctxs = json.loads(answer.read())
    ranked = sorted(
        zip(meta["universe"], ctxs), key=lambda pair: float(pair[1].get("dayNtlVlm") or 0), reverse=True
    )
    return [perp["name"] for perp, _ in ranked if not perp.get("isDelisted")][:count]


def main():
    out = flag("--out", "../data/frames.jsonl.gz")
    per_channel = int(flag("--per-channel", "2000"))
    max_secs = int(flag("--max-secs", "1200"))
    book_coins = busiest_perps(int(flag("--books", "150")))

    counts = {channel: 0 for channel in CHANNELS}
    lines = []
    done = threading.Event()

    def on_open(ws):
        for coin in TRADE_COINS:
            ws.send(json.dumps({"method": "subscribe", "subscription": {"type": "trades", "coin": coin}}))
        for coin in book_coins:
            ws.send(json.dumps({"method": "subscribe", "subscription": {"type": "l2Book", "coin": coin}}))

    def on_message(ws, text):
        try:
            channel = json.loads(text).get("channel")
        except ValueError:
            return
        if channel in counts and counts[channel] < per_channel:
            counts[channel] += 1
            lines.append(text)
        if all(count >= per_channel for count in counts.values()):
            done.set()
            ws.close()

    def ping(ws):
        while not done.wait(30):
            try:
                ws.send(json.dumps({"method": "ping"}))
            except Exception:  # noqa: BLE001 - the socket closed; the main thread decides
                return

    ws = websocket.WebSocketApp(URL, on_open=on_open, on_message=on_message)
    runner = threading.Thread(target=ws.run_forever, daemon=True)
    runner.start()
    threading.Thread(target=ping, args=(ws,), daemon=True).start()
    started = time.monotonic()
    while not done.wait(5):
        print(f"{int(time.monotonic() - started)} s: {counts}", file=sys.stderr)
        if time.monotonic() - started > max_secs:
            ws.close()
            break

    # In arrival order.
    with gzip.open(out, "wt", encoding="utf-8", compresslevel=9) as file:
        for line in lines:
            file.write(line + "\n")
    print(f"wrote {len(lines)} frames to {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
