#!/usr/bin/env python3
"""
Outbox coordination tuning using live lease telemetry.

What it does:
1) Sends concurrent POST /outbox/lease requests with candidate lease_ttl_ms values.
2) Reads GET /outbox/lease/metrics before/after each candidate.
3) Recommends lease_ttl/window/deferral env values based on observed contention.
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib import error, request


def percentile(values: List[float], p: float) -> float:
    if not values:
        return 0.0
    if len(values) == 1:
        return float(values[0])
    vals = sorted(values)
    pos = (len(vals) - 1) * (p / 100.0)
    low = int(pos)
    high = min(low + 1, len(vals) - 1)
    w = pos - low
    return vals[low] * (1 - w) + vals[high] * w


def clamp(value: int, low: int, high: int) -> int:
    return max(low, min(high, value))


def read_api_key(explicit: str) -> str:
    if explicit.strip():
        return explicit.strip()

    env_file = Path(__file__).resolve().parents[1] / ".env"
    if env_file.exists():
        for raw in env_file.read_text(encoding="utf-8").splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("GITGOV_API_KEY="):
                value = line.split("=", 1)[1].strip().strip('"').strip("'")
                if value:
                    return value
    raise RuntimeError("Missing API key: pass --api-key or define GITGOV_API_KEY in gitgov-server/.env")


def http_json(
    *,
    base_url: str,
    api_key: str,
    endpoint: str,
    method: str,
    payload: Optional[Dict[str, Any]],
    timeout_sec: float,
) -> Dict[str, Any]:
    url = base_url.rstrip("/") + endpoint
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = request.Request(url=url, method=method, data=body)
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")

    with request.urlopen(req, timeout=timeout_sec) as resp:
        data = resp.read().decode("utf-8")
        if not data:
            return {}
        return json.loads(data)


@dataclass
class LeaseResult:
    http_code: int
    latency_ms: float
    granted: bool
    wait_ms: int
    mode: str
    error: Optional[str] = None


def send_lease_request(
    *,
    base_url: str,
    api_key: str,
    timeout_sec: float,
    scope: str,
    holder: str,
    lease_ttl_ms: int,
) -> LeaseResult:
    url = base_url.rstrip("/") + "/outbox/lease"
    payload = {
        "scope": scope,
        "holder": holder,
        "lease_ttl_ms": lease_ttl_ms,
        "max_wait_ms": lease_ttl_ms,
    }
    req = request.Request(
        url=url,
        method="POST",
        data=json.dumps(payload).encode("utf-8"),
    )
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")

    started = time.perf_counter()
    try:
        with request.urlopen(req, timeout=timeout_sec) as resp:
            elapsed_ms = (time.perf_counter() - started) * 1000.0
            body = json.loads(resp.read().decode("utf-8"))
            return LeaseResult(
                http_code=int(resp.status),
                latency_ms=elapsed_ms,
                granted=bool(body.get("granted", False)),
                wait_ms=int(body.get("wait_ms", 0) or 0),
                mode=str(body.get("mode", "unknown")),
                error=None,
            )
    except error.HTTPError as e:
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        detail = ""
        try:
            detail = e.read().decode("utf-8")
        except Exception:  # noqa: BLE001
            detail = str(e)
        return LeaseResult(
            http_code=int(e.code),
            latency_ms=elapsed_ms,
            granted=False,
            wait_ms=0,
            mode="http_error",
            error=detail,
        )
    except Exception as e:  # noqa: BLE001
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        return LeaseResult(
            http_code=0,
            latency_ms=elapsed_ms,
            granted=False,
            wait_ms=0,
            mode="network_error",
            error=str(e),
        )


def summarize_results(results: List[LeaseResult], lease_ttl_ms: int) -> Dict[str, Any]:
    total = len(results)
    ok_200 = sum(1 for r in results if r.http_code == 200)
    granted = sum(1 for r in results if r.granted)
    denied = sum(1 for r in results if (r.http_code == 200 and not r.granted))

    waits = [float(r.wait_ms) for r in results if r.http_code == 200]
    denied_waits = [float(r.wait_ms) for r in results if (r.http_code == 200 and not r.granted)]
    lat = [r.latency_ms for r in results]

    http_counts: Dict[str, int] = {}
    mode_counts: Dict[str, int] = {}
    for r in results:
        http_counts[str(r.http_code)] = http_counts.get(str(r.http_code), 0) + 1
        mode_counts[r.mode] = mode_counts.get(r.mode, 0) + 1

    return {
        "lease_ttl_ms": lease_ttl_ms,
        "total_requests": total,
        "ok_200": ok_200,
        "ok_200_ratio": (ok_200 / total) if total else 0.0,
        "granted": granted,
        "denied": denied,
        "denied_ratio": (denied / total) if total else 0.0,
        "http_counts": http_counts,
        "mode_counts": mode_counts,
        "wait_ms": {
            "avg": statistics.mean(waits) if waits else 0.0,
            "p50": percentile(waits, 50),
            "p95": percentile(waits, 95),
            "p99": percentile(waits, 99),
            "max": max(waits) if waits else 0.0,
            "denied_avg": statistics.mean(denied_waits) if denied_waits else 0.0,
            "denied_p95": percentile(denied_waits, 95),
        },
        "latency_ms": {
            "avg": statistics.mean(lat) if lat else 0.0,
            "p95": percentile(lat, 95),
            "p99": percentile(lat, 99),
            "max": max(lat) if lat else 0.0,
        },
    }


def metrics_delta(before: Dict[str, Any], after: Dict[str, Any]) -> Dict[str, Any]:
    b = before.get("telemetry", {})
    a = after.get("telemetry", {})

    def diff(key: str) -> int:
        return int(a.get(key, 0) or 0) - int(b.get(key, 0) or 0)

    return {
        "total_requests": diff("total_requests"),
        "granted_requests": diff("granted_requests"),
        "denied_requests": diff("denied_requests"),
        "fail_open_disabled_requests": diff("fail_open_disabled_requests"),
        "fail_open_db_error_requests": diff("fail_open_db_error_requests"),
        "ttl_clamped_requests": diff("ttl_clamped_requests"),
        "wait_clamped_requests": diff("wait_clamped_requests"),
        "avg_wait_ms_after": int(a.get("avg_wait_ms", 0) or 0),
        "max_wait_ms_after": int(a.get("max_wait_ms", 0) or 0),
    }


def tune_from_best(best: Dict[str, Any]) -> Dict[str, int]:
    ttl = int(best["lease_ttl_ms"])
    denied_ratio = float(best["denied_ratio"])
    denied_p95 = float(best["wait_ms"]["denied_p95"])

    if denied_ratio >= 0.50 or denied_p95 >= ttl * 0.90:
        window_factor = 10
        deferral_factor = 0.70
    elif denied_ratio >= 0.30 or denied_p95 >= ttl * 0.75:
        window_factor = 8
        deferral_factor = 0.60
    elif denied_ratio >= 0.15:
        window_factor = 7
        deferral_factor = 0.50
    else:
        window_factor = 6
        deferral_factor = 0.40

    window_ms = clamp(window_factor * ttl, 5_000, 300_000)
    deferral_from_ttl = int(ttl * deferral_factor)
    deferral_from_wait = int(denied_p95 * 0.80)
    deferral_ms = max(deferral_from_ttl, deferral_from_wait)
    deferral_ms = clamp(deferral_ms, 500, max(500, window_ms - 1_000))

    return {
        "GITGOV_OUTBOX_SERVER_LEASE_TTL_MS": clamp(ttl, 1_000, 60_000),
        "GITGOV_OUTBOX_GLOBAL_COORD_WINDOW_MS": window_ms,
        "GITGOV_OUTBOX_GLOBAL_COORD_MAX_DEFERRAL_MS": deferral_ms,
    }


def score(summary: Dict[str, Any]) -> float:
    denied_ratio = float(summary["denied_ratio"])
    wait_p95 = float(summary["wait_ms"]["p95"])
    latency_p95 = float(summary["latency_ms"]["p95"])
    ok_200_ratio = float(summary.get("ok_200_ratio", 0.0))
    sample_penalty = max(0.0, 0.90 - ok_200_ratio) * 50_000.0
    return sample_penalty + denied_ratio * 10_000.0 + wait_p95 * 0.8 + latency_p95 * 0.2


def run_one_candidate(
    *,
    base_url: str,
    api_key: str,
    timeout_sec: float,
    requests_count: int,
    concurrency: int,
    holders: int,
    scope: str,
    lease_ttl_ms: int,
) -> Dict[str, Any]:
    before = http_json(
        base_url=base_url,
        api_key=api_key,
        endpoint="/outbox/lease/metrics",
        method="GET",
        payload=None,
        timeout_sec=timeout_sec,
    )

    started = time.perf_counter()
    results: List[LeaseResult] = []
    with ThreadPoolExecutor(max_workers=max(1, concurrency)) as executor:
        futures = []
        for i in range(requests_count):
            holder = f"holder-{(i % max(1, holders))}"
            futures.append(
                executor.submit(
                    send_lease_request,
                    base_url=base_url,
                    api_key=api_key,
                    timeout_sec=timeout_sec,
                    scope=scope,
                    holder=holder,
                    lease_ttl_ms=lease_ttl_ms,
                )
            )
        for future in as_completed(futures):
            results.append(future.result())
    ended = time.perf_counter()

    after = http_json(
        base_url=base_url,
        api_key=api_key,
        endpoint="/outbox/lease/metrics",
        method="GET",
        payload=None,
        timeout_sec=timeout_sec,
    )

    summary = summarize_results(results, lease_ttl_ms)
    summary["duration_sec"] = ended - started
    summary["throughput_rps"] = requests_count / max(ended - started, 1e-9)
    summary["server_metrics_delta"] = metrics_delta(before, after)
    summary["score"] = score(summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Tune outbox lease/window/deferral with live telemetry")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--api-key", default="")
    parser.add_argument("--ttl-candidates", default="3000,5000,8000")
    parser.add_argument("--requests-per-ttl", type=int, default=180)
    parser.add_argument("--concurrency", type=int, default=12)
    parser.add_argument("--holders", type=int, default=16)
    parser.add_argument("--scope", default="tuning-live")
    parser.add_argument("--timeout-sec", type=float, default=10.0)
    parser.add_argument("--cooldown-ms", type=int, default=250)
    parser.add_argument("--out-json", default="")
    args = parser.parse_args()

    api_key = read_api_key(args.api_key)
    ttl_candidates = [
        int(x.strip())
        for x in args.ttl_candidates.split(",")
        if x.strip()
    ]
    if not ttl_candidates:
        raise RuntimeError("No TTL candidates provided")

    print("Outbox coordination tuning")
    print(f"  server_url        : {args.server_url}")
    print(f"  ttl_candidates_ms : {ttl_candidates}")
    print(f"  requests_per_ttl  : {args.requests_per_ttl}")
    print(f"  concurrency       : {args.concurrency}")
    print(f"  holders           : {args.holders}")
    print("")

    runs: List[Dict[str, Any]] = []

    for ttl in ttl_candidates:
        print(f"Running candidate lease_ttl_ms={ttl} ...")
        run = run_one_candidate(
            base_url=args.server_url,
            api_key=api_key,
            timeout_sec=args.timeout_sec,
            requests_count=args.requests_per_ttl,
            concurrency=args.concurrency,
            holders=args.holders,
            scope=args.scope,
            lease_ttl_ms=ttl,
        )
        runs.append(run)
        print(
            "  denied_ratio={:.3f} wait_p95={:.1f}ms latency_p95={:.1f}ms score={:.1f}".format(
                run["denied_ratio"],
                run["wait_ms"]["p95"],
                run["latency_ms"]["p95"],
                run["score"],
            )
        )
        if args.cooldown_ms > 0:
            time.sleep(args.cooldown_ms / 1000.0)

    valid_runs = [r for r in runs if r.get("ok_200_ratio", 0.0) >= 0.90]
    if valid_runs:
        best = min(valid_runs, key=score)
    else:
        best = min(runs, key=score)
    recommended = tune_from_best(best)

    report = {
        "timestamp_ms": int(time.time() * 1000),
        "server_url": args.server_url,
        "scope": args.scope,
        "requests_per_ttl": args.requests_per_ttl,
        "concurrency": args.concurrency,
        "holders": args.holders,
        "runs": runs,
        "best": best,
        "recommended_env": recommended,
    }

    print("\nBest candidate:")
    print(
        "  lease_ttl_ms={} denied_ratio={:.3f} wait_p95={:.1f}ms".format(
            best["lease_ttl_ms"],
            best["denied_ratio"],
            best["wait_ms"]["p95"],
        )
    )
    print("\nRecommended env:")
    for key, value in recommended.items():
        print(f"  {key}={value}")

    if args.out_json.strip():
        out_path = Path(args.out_json).resolve()
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"\nWrote report: {out_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
