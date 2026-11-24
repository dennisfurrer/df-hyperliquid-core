"""Runs every benchmark in bench/ and writes results/<date>.json and results/<date>.md.

Started by run.sh, which builds the Rust binaries and the Python environment first. Steps:

1. Signature check: the three SDKs sign the same order with the same key and nonce; the run
   stops if any signature differs.
2. Signing time, offline, one SDK after another.
3. Websocket parse throughput over the recorded corpus in data/.
4. Info and exchange round trips on mainnet: one long-lived worker process per SDK, driven
   in paced, interleaved rounds so the three share network conditions.
5. Live websocket: the three SDKs subscribe at the same time for the same 60 seconds.

Only the standard library is used here; the SDKs run in their own processes.
"""

import datetime
import json
import math
import os
import platform
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

BENCH = Path(__file__).resolve().parent
PY = BENCH / "py"
DF = BENCH / "target" / "release" / "bench-df"
HYPERSDK = BENCH / "target" / "release" / "bench-hypersdk"
CORPUS = BENCH / "data" / "frames.jsonl.gz"

ROUNDS = int(os.environ.get("BENCH_ROUNDS", "100"))
WARMUP_ROUNDS = 3
GAP_S = float(os.environ.get("BENCH_GAP_S", "0.3"))
LIVE_SECS = int(os.environ.get("BENCH_LIVE_SECS", "60"))
PY_SIGN_SAMPLES_NATIVE = 2_000
PY_SIGN_SAMPLES_COINCURVE = 5_000

NATIVE = "eth_keys.backends.NativeECCBackend"
COINCURVE = "eth_keys.backends.CoinCurveECCBackend"

DF_NAME = "df-hyperliquid-core"
HYPERSDK_NAME = "hypersdk"
PY_NAME = "hyperliquid-python-sdk"
PY_FAST_NAME = "hyperliquid-python-sdk + coincurve"


def log(text):
    print(text, file=sys.stderr, flush=True)


def py_command(*args):
    return ["uv", "run", "--project", str(PY), "--frozen", "python", str(PY / "bench_py.py"), *args]


def py_env(backend):
    env = dict(os.environ)
    env["ECC_BACKEND_CLASS"] = backend
    return env


def run_json(command, env=None):
    done = subprocess.run(command, capture_output=True, text=True, env=env, check=False)
    if done.returncode != 0:
        raise SystemExit(f"{' '.join(map(str, command))} failed:\n{done.stderr}")
    return json.loads(done.stdout.strip().splitlines()[-1])


def capture(command, cwd=None):
    try:
        return subprocess.run(command, capture_output=True, text=True, cwd=cwd, check=False).stdout.strip()
    except OSError:
        return ""


def machine():
    sysctl = lambda name: capture(["sysctl", "-n", name])  # noqa: E731
    memory = sysctl("hw.memsize")
    return {
        "model": sysctl("hw.model"),
        "cpu": sysctl("machdep.cpu.brand_string"),
        "cores": sysctl("hw.ncpu"),
        "memory_gb": int(memory) // 2**30 if memory.isdigit() else None,
        "os": f"{capture(['sw_vers', '-productName'])} {capture(['sw_vers', '-productVersion'])}",
        "rustc": capture(["rustc", "--version"], cwd=BENCH),
        "python": capture(["uv", "run", "--project", str(PY), "--frozen", "python", "--version"]),
        "location": os.environ.get("BENCH_LOCATION", "not given"),
    }


def locked_version(name):
    lock = (BENCH / "Cargo.lock").read_text().split("[[package]]")
    for block in lock:
        if f'name = "{name}"\n' in block:
            for line in block.splitlines():
                if line.startswith("version = "):
                    return line.split('"')[1]
    return None


def versions():
    py_sdk = capture(
        [
            "uv", "run", "--project", str(PY), "--frozen", "python", "-c",
            "import importlib.metadata as m; print(m.version('hyperliquid-python-sdk'), "
            "m.version('eth-account'), m.version('eth-keys'), m.version('coincurve'), m.version('requests'), "
            "m.version('websocket-client'))",
        ]
    ).split()
    return {
        DF_NAME: locked_version("df-hyperliquid-core"),
        HYPERSDK_NAME: locked_version("hypersdk"),
        PY_NAME: py_sdk[0] if py_sdk else None,
        "eth-account": py_sdk[1] if len(py_sdk) > 1 else None,
        "eth-keys": py_sdk[2] if len(py_sdk) > 2 else None,
        "coincurve": py_sdk[3] if len(py_sdk) > 3 else None,
        "requests": py_sdk[4] if len(py_sdk) > 4 else None,
        "websocket-client": py_sdk[5] if len(py_sdk) > 5 else None,
        "git_commit": capture(["git", "rev-parse", "--short", "HEAD"], cwd=BENCH),
    }


def signature_check():
    checks = [
        run_json([str(DF), "sign-check"]),
        run_json([str(HYPERSDK), "sign-check"]),
        run_json(py_command("sign-check"), env=py_env(NATIVE)),
        run_json(py_command("sign-check"), env=py_env(COINCURVE)),
    ]
    signatures = {check["signature"] for check in checks}
    addresses = {check["address"] for check in checks}
    actions = {json.dumps(check["action"], sort_keys=True) for check in checks}
    same = len(signatures) == 1 and len(addresses) == 1 and len(actions) == 1
    for check in checks:
        name = check["sdk"] + (f" ({check['backend']})" if "backend" in check else "")
        log(f"  {name:62} {check['signature']}")
    log(f"signature check: {'all equal' if same else 'MISMATCH'}")
    if not same:
        raise SystemExit("the SDKs signed the fixed order differently; not timing anything")
    return {"equal": same, "signature": signatures.pop(), "address": addresses.pop(), "checks": checks}


def signing():
    log("signing: df-hyperliquid-core")
    df = run_json([str(DF), "sign"])
    log("signing: hypersdk")
    hyper = run_json([str(HYPERSDK), "sign"])
    log("signing: hyperliquid-python-sdk (pure Python ECDSA, the default install)")
    native = run_json(py_command("sign", "--samples", str(PY_SIGN_SAMPLES_NATIVE)), env=py_env(NATIVE))
    log("signing: hyperliquid-python-sdk with coincurve")
    fast = run_json(py_command("sign", "--samples", str(PY_SIGN_SAMPLES_COINCURVE)), env=py_env(COINCURVE))
    return {DF_NAME: df, HYPERSDK_NAME: hyper, PY_NAME: native, PY_FAST_NAME: fast}


def parsing():
    log("parse: decoding the recorded corpus")
    return {
        DF_NAME: run_json([str(DF), "parse", "--corpus", str(CORPUS)]),
        HYPERSDK_NAME: run_json([str(HYPERSDK), "parse", "--corpus", str(CORPUS)]),
        PY_NAME: run_json(py_command("parse", "--corpus", str(CORPUS)), env=py_env(NATIVE)),
    }


def order_price():
    """A post-only buy 30% under the BTC mid on whole dollars, and a size worth about $12."""
    request = urllib.request.Request(
        "https://api.hyperliquid.xyz/info",
        data=json.dumps({"type": "allMids"}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=10) as answer:
        pop = answer.headers.get("X-Amz-Cf-Pop", "")
        mid = float(json.loads(answer.read())["BTC"])
    px = int(float(f"{mid * 0.7:.5g}"))
    sz = math.ceil(12 / px * 1e5) / 1e5
    return str(px), f"{sz:.5f}", pop


class Worker:
    """One SDK's long-lived process: a line in, one JSON line out."""

    def __init__(self, name, command, env=None):
        self.name = name
        self.process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env
        )

    def ask(self, command):
        self.process.stdin.write(command + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise SystemExit(f"{self.name} worker stopped: {self.process.stderr.read()}")
        return json.loads(line)

    def stop(self):
        try:
            self.process.stdin.write("quit\n")
            self.process.stdin.flush()
        except BrokenPipeError:
            pass
        self.process.wait(timeout=30)


def summary_ms(samples_ns):
    ordered = sorted(samples_ns)
    if not ordered:
        return {}

    def at(fraction):
        return ordered[min(round((len(ordered) - 1) * fraction), len(ordered) - 1)] / 1e6

    return {
        "n": len(ordered),
        "min_ms": ordered[0] / 1e6,
        "median_ms": at(0.5),
        "p90_ms": at(0.9),
        "p99_ms": at(0.99),
        "max_ms": ordered[-1] / 1e6,
        "mean_ms": sum(ordered) / len(ordered) / 1e6,
    }


def network():
    px, sz, pop = order_price()
    log(f"network: post-only BTC buy {sz} at {px}; edge {pop}; {ROUNDS} rounds, {GAP_S}s between requests")
    workers = [
        Worker(DF_NAME, [str(DF), "serve", "--px", px, "--sz", sz]),
        Worker(HYPERSDK_NAME, [str(HYPERSDK), "serve", "--px", px, "--sz", sz]),
        Worker(PY_NAME, py_command("serve", "--px", px, "--sz", sz), env=py_env(NATIVE)),
    ]
    results = {
        worker.name: {"address": worker.ask("address")["answer"], "info": [], "exchange": [], "warmup": []}
        for worker in workers
    }
    try:
        for round_index in range(WARMUP_ROUNDS + ROUNDS):
            # Rotate who goes first so no SDK always follows the same neighbour.
            shift = round_index % len(workers)
            order = workers[shift:] + workers[:shift]
            for test in ("info", "exchange"):
                for worker in order:
                    answer = worker.ask(test)
                    answer["round"] = round_index
                    answer["test"] = test
                    if round_index < WARMUP_ROUNDS:
                        results[worker.name]["warmup"].append(answer)
                    else:
                        results[worker.name][test].append(answer)
                    time.sleep(GAP_S)
            if (round_index + 1) % 10 == 0:
                log(f"  round {round_index + 1}/{WARMUP_ROUNDS + ROUNDS}")
    finally:
        for worker in workers:
            worker.stop()
    for name, result in results.items():
        for test in ("info", "exchange"):
            answers = result[test]
            ok = [answer["ns"] for answer in answers if answer["ok"]]
            result[f"{test}_summary"] = summary_ms(ok)
            result[f"{test}_failed"] = len(answers) - len(ok)
            result[f"{test}_first_answer"] = answers[0]["answer"] if answers else None
            result[f"{test}_samples_ns"] = [answer["ns"] for answer in answers]
            result[f"{test}_ok"] = [answer["ok"] for answer in answers]
            del result[test]
    return {"order": {"coin": "BTC", "side": "buy", "tif": "Alo", "px": px, "sz": sz}, "edge_pop": pop,
            "rounds": ROUNDS, "warmup_rounds": WARMUP_ROUNDS, "gap_s": GAP_S, "sdks": results}


def live():
    log(f"live: the three SDKs subscribe to BTC l2Book and trades at the same time for {LIVE_SECS} s")
    secs = str(LIVE_SECS)
    commands = {
        DF_NAME: ([str(DF), "live", "--secs", secs], None),
        HYPERSDK_NAME: ([str(HYPERSDK), "live", "--secs", secs], None),
        PY_NAME: (py_command("live", "--secs", secs), py_env(NATIVE)),
    }
    running = {
        name: subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        for name, (command, env) in commands.items()
    }
    results = {}
    for name, process in running.items():
        out, err = process.communicate(timeout=LIVE_SECS + 120)
        if process.returncode != 0:
            raise SystemExit(f"{name} live run failed:\n{err}")
        results[name] = json.loads(out.strip().splitlines()[-1])["live"]
    return results


def us(ns):
    return f"{ns / 1000:.1f}"


def markdown(report):
    lines = []
    sign = report["signing"]
    lines.append("### Signing one BTC limit order (offline)\n")
    lines.append(f"Same signature from all SDKs: {'yes' if report['signature_check']['equal'] else 'no'} "
                 f"(`{report['signature_check']['signature'][:18]}...`)\n")
    lines.append("| SDK | hash + sign median (us) | p99 (us) | ops/s | build + sign + body median (us) | p99 (us) | ops/s |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for name, row in sign.items():
        a, b = row["sign"], row["build_sign_body"]
        lines.append(f"| {name} | {us(a['median_ns'])} | {us(a['p99_ns'])} | {a['ops_per_s']:,.0f} | "
                     f"{us(b['median_ns'])} | {us(b['p99_ns'])} | {b['ops_per_s']:,.0f} |")

    net = report["network"]
    for test, title in (("info", "Info round trip: POST /info l2Book BTC"),
                        ("exchange", "Exchange round trip: sign and POST /exchange, refused order")):
        lines.append(f"\n### {title} ({net['rounds']} requests per SDK)\n")
        lines.append("| SDK | median (ms) | p90 (ms) | p99 (ms) | min (ms) | max (ms) | failed |")
        lines.append("|---|---:|---:|---:|---:|---:|---:|")
        for name, row in net["sdks"].items():
            s = row[f"{test}_summary"]
            if not s:
                lines.append(f"| {name} | - | - | - | - | - | {row[f'{test}_failed']} |")
                continue
            lines.append(f"| {name} | {s['median_ms']:.2f} | {s['p90_ms']:.2f} | {s['p99_ms']:.2f} | "
                         f"{s['min_ms']:.2f} | {s['max_ms']:.2f} | {row[f'{test}_failed']} |")

    lines.append(f"\n### Websocket parse throughput ({report['parsing'][DF_NAME]['parse']['all']['frames']} recorded frames)\n")
    lines.append("| SDK | all frames/s | all MB/s | l2Book frames/s | trades frames/s | ns per frame | frames not read |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for name, row in report["parsing"].items():
        p = row["parse"]
        lines.append(f"| {name} | {p['all']['frames_per_s']:,.0f} | {p['all']['mb_per_s']:,.1f} | "
                     f"{p['l2Book']['frames_per_s']:,.0f} | {p['trades']['frames_per_s']:,.0f} | "
                     f"{p['all']['ns_per_frame']:,.0f} | {p['all']['failed']} |")

    lines.append(f"\n### Live websocket, BTC l2Book and trades, {LIVE_SECS} s, all three at once\n")
    lines.append("| SDK | l2Book frames | trades frames | trades | frames not read | age median (ms) | age p99 (ms) |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for name, row in report["live"].items():
        unreadable = "not reported" if row["unreadable"] is None else row["unreadable"]
        lines.append(f"| {name} | {row['book_frames']} | {row['trade_frames']} | {row['trades']} | {unreadable} | "
                     f"{row['age_ms_median']} | {row['age_ms_p99']} |")
    return "\n".join(lines) + "\n"


def main():
    if not CORPUS.exists():
        raise SystemExit(f"{CORPUS} is missing; record it with: cd py && uv run python record.py")
    started = datetime.datetime.now(datetime.timezone.utc)
    report = {"started_utc": started.isoformat(timespec="seconds"), "machine": machine(), "versions": versions()}
    log(json.dumps(report["machine"]))
    log(json.dumps(report["versions"]))
    report["signature_check"] = signature_check()
    report["signing"] = signing()
    report["parsing"] = parsing()
    report["network"] = network()
    report["live"] = live()
    report["platform"] = platform.platform()

    results = BENCH / "results"
    results.mkdir(exist_ok=True)
    stem = started.strftime("%Y-%m-%d")
    (results / f"{stem}.json").write_text(json.dumps(report, indent=1) + "\n")
    table = markdown(report)
    (results / f"{stem}.md").write_text(table)
    print(table)
    log(f"wrote results/{stem}.json and results/{stem}.md")


if __name__ == "__main__":
    main()
