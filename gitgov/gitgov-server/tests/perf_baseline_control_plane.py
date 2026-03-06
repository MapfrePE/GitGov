#!/usr/bin/env python3
"""
Control Plane baseline benchmark (Phase 0).

Measures per endpoint:
- latency (min/avg/p50/p95/p99/max)
- throughput (req/s)
- HTTP distribution (includes 401/429/5xx counters)

Endpoints:
- POST /events
- GET  /logs
- GET  /stats
- POST /chat/ask
"""

from __future__ import annotations

import argparse
import json
import random
import statistics
import time
import uuid
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from urllib import error, request


@dataclass
class EndpointResult:
    endpoint: str
    http_code: int
    latency_ms: float
    error_kind: Optional[str] = None
    error_message: Optional[str] = None


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


def read_api_key(explicit: str) -> str:
    if explicit.strip():
        return explicit.strip()
    env_file = Path(__file__).resolve().parents[1] / ".env"
    if env_file.exists():
        for line in env_file.read_text(encoding="utf-8").splitlines():
            clean = line.strip()
            if clean.startswith("GITGOV_API_KEY="):
                value = clean.split("=", 1)[1].strip()
                if value:
                    return value
    raise RuntimeError("Missing API key: pass --api-key or define GITGOV_API_KEY in gitgov-server/.env")


def do_http(
    *,
    base_url: str,
    api_key: str,
    endpoint: str,
    method: str,
    payload: Optional[Dict[str, Any]],
    timeout_sec: float,
) -> EndpointResult:
    url = base_url.rstrip("/") + endpoint
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = request.Request(url=url, method=method, data=body)
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")

    started = time.perf_counter()
    try:
        with request.urlopen(req, timeout=timeout_sec) as resp:
            _ = resp.read()
            elapsed_ms = (time.perf_counter() - started) * 1000.0
            return EndpointResult(endpoint=endpoint, http_code=int(resp.status), latency_ms=elapsed_ms)
    except error.HTTPError as e:
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        return EndpointResult(
            endpoint=endpoint,
            http_code=int(e.code),
            latency_ms=elapsed_ms,
            error_kind="http_error",
            error_message=str(e),
        )
    except Exception as e:  # noqa: BLE001
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        lowered = str(e).lower()
        kind = "timeout" if "timed out" in lowered else "network_error"
        return EndpointResult(
            endpoint=endpoint,
            http_code=0,
            latency_ms=elapsed_ms,
            error_kind=kind,
            error_message=str(e),
        )


def events_payload() -> Dict[str, Any]:
    now_ms = int(time.time() * 1000)
    return {
        "events": [
            {
                "event_uuid": str(uuid.uuid4()),
                "event_type": "commit",
                "user_login": "perf_baseline_user",
                "status": "success",
                "files": [],
                "timestamp": now_ms,
                "metadata": {"suite": "perf_baseline_control_plane"},
            }
        ],
        "client_version": "perf-baseline-v1",
    }


CHAT_QUESTIONS = [
    "quien hizo push a main esta semana sin ticket de jira?",
    "cuantos pushes bloqueados tuvo el equipo este mes?",
    "que rol tiene el usuario mapfrepe?",
    "pushes sin ticket del usuario mapfrepe",
    "en que me puedes ayudar?",
]


def chat_payload(org_name: Optional[str]) -> Dict[str, Any]:
    return {"question": random.choice(CHAT_QUESTIONS), "org_name": org_name}


def summarize(name: str, results: List[EndpointResult], started: float, ended: float) -> Dict[str, Any]:
    lat = [r.latency_ms for r in results]
    http_counts: Dict[str, int] = {}
    error_counts: Dict[str, int] = {}
    count_401 = 0
    count_429 = 0
    count_5xx = 0

    for r in results:
        code = str(r.http_code)
        http_counts[code] = http_counts.get(code, 0) + 1
        if r.http_code == 401:
            count_401 += 1
        if r.http_code == 429:
            count_429 += 1
        if 500 <= r.http_code <= 599:
            count_5xx += 1
        if r.error_kind:
            error_counts[r.error_kind] = error_counts.get(r.error_kind, 0) + 1

    duration = max(ended - started, 1e-9)
    return {
        "endpoint": name,
        "requests": len(results),
        "duration_sec": duration,
        "throughput_rps": len(results) / duration,
        "http_counts": http_counts,
        "error_counts": error_counts,
        "latency_ms": {
            "min": min(lat) if lat else 0.0,
            "avg": statistics.mean(lat) if lat else 0.0,
            "p50": percentile(lat, 50),
            "p95": percentile(lat, 95),
            "p99": percentile(lat, 99),
            "max": max(lat) if lat else 0.0,
        },
        "error_rate": {
            "401": count_401 / len(results) if results else 0.0,
            "429": count_429 / len(results) if results else 0.0,
            "5xx": count_5xx / len(results) if results else 0.0,
        },
    }


def run_workload(
    *,
    name: str,
    base_url: str,
    api_key: str,
    requests_count: int,
    concurrency: int,
    timeout_sec: float,
    endpoint: str,
    method: str,
    payload_factory: Optional[callable],
) -> Dict[str, Any]:
    started = time.perf_counter()
    results: List[EndpointResult] = []
    with ThreadPoolExecutor(max_workers=max(1, concurrency)) as executor:
        futures = []
        for _ in range(requests_count):
            payload = payload_factory() if payload_factory else None
            futures.append(
                executor.submit(
                    do_http,
                    base_url=base_url,
                    api_key=api_key,
                    endpoint=endpoint,
                    method=method,
                    payload=payload,
                    timeout_sec=timeout_sec,
                )
            )
        for future in as_completed(futures):
            results.append(future.result())
    ended = time.perf_counter()
    return summarize(name, results, started, ended)


def print_summary(result: Dict[str, Any]) -> None:
    lat = result["latency_ms"]
    print(f"[{result['endpoint']}] req={result['requests']} throughput={result['throughput_rps']:.2f} rps")
    print(
        f"  latency ms: min={lat['min']:.1f} avg={lat['avg']:.1f} "
        f"p50={lat['p50']:.1f} p95={lat['p95']:.1f} p99={lat['p99']:.1f} max={lat['max']:.1f}"
    )
    print(f"  http: {result['http_counts']}")
    print(
        "  error_rate: "
        f"401={result['error_rate']['401']:.3f}, "
        f"429={result['error_rate']['429']:.3f}, "
        f"5xx={result['error_rate']['5xx']:.3f}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="GitGov Control Plane baseline benchmark")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--api-key", default="")
    parser.add_argument("--org-name", default=None)
    parser.add_argument("--requests", type=int, default=40)
    parser.add_argument("--concurrency", type=int, default=4)
    parser.add_argument("--timeout-sec", type=float, default=12.0)
    parser.add_argument("--out-json", default="")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    random.seed(args.seed)
    api_key = read_api_key(args.api_key)

    print("Running Control Plane baseline benchmark...")
    print(f"  server_url  : {args.server_url}")
    print(f"  requests    : {args.requests}")
    print(f"  concurrency : {args.concurrency}")
    print(f"  timeout_sec : {args.timeout_sec}")
    print("")

    workloads: List[Tuple[str, Dict[str, Any]]] = []

    workloads.append(
        (
            "POST /events",
            run_workload(
                name="POST /events",
                base_url=args.server_url,
                api_key=api_key,
                requests_count=args.requests,
                concurrency=args.concurrency,
                timeout_sec=args.timeout_sec,
                endpoint="/events",
                method="POST",
                payload_factory=events_payload,
            ),
        )
    )
    workloads.append(
        (
            "GET /logs",
            run_workload(
                name="GET /logs",
                base_url=args.server_url,
                api_key=api_key,
                requests_count=args.requests,
                concurrency=args.concurrency,
                timeout_sec=args.timeout_sec,
                endpoint="/logs?limit=50&offset=0",
                method="GET",
                payload_factory=None,
            ),
        )
    )
    workloads.append(
        (
            "GET /stats",
            run_workload(
                name="GET /stats",
                base_url=args.server_url,
                api_key=api_key,
                requests_count=args.requests,
                concurrency=args.concurrency,
                timeout_sec=args.timeout_sec,
                endpoint="/stats",
                method="GET",
                payload_factory=None,
            ),
        )
    )
    workloads.append(
        (
            "POST /chat/ask",
            run_workload(
                name="POST /chat/ask",
                base_url=args.server_url,
                api_key=api_key,
                requests_count=args.requests,
                concurrency=args.concurrency,
                timeout_sec=args.timeout_sec,
                endpoint="/chat/ask",
                method="POST",
                payload_factory=lambda: chat_payload(args.org_name),
            ),
        )
    )

    summary = {
        "generated_at": int(time.time() * 1000),
        "server_url": args.server_url,
        "requests_per_endpoint": args.requests,
        "concurrency": args.concurrency,
        "timeout_sec": args.timeout_sec,
        "endpoints": {name: payload for name, payload in workloads},
    }

    print("")
    print("=== Baseline Summary ===")
    for _, payload in workloads:
        print_summary(payload)
        print("")

    if args.out_json:
        out_path = Path(args.out_json)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
        print(f"Saved JSON report to: {out_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

