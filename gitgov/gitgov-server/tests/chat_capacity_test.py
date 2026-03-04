#!/usr/bin/env python3
"""
Chat capacity benchmark for GitGov Control Plane.

Measures:
- HTTP success/error distribution
- Chat response status distribution (ok/insufficient_data/feature_not_available/error)
- Latency percentiles (p50/p95/p99)
- Throughput (requests/sec)
- Timeout and network failures

Usage example:
  API_KEY=... python tests/chat_capacity_test.py --requests 120 --concurrency 12 --scenario mixed
"""

from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib import request, error


DEFAULT_SCENARIOS: Dict[str, List[str]] = {
    "deterministic": [
        "¿Quién hizo push a main esta semana sin ticket de Jira?",
        "¿Cuántos pushes bloqueados tuvo el equipo este mes?",
        "Muéstrame todos los commits de dev1 entre 2026-01-01 y 2026-03-01",
        "y del usuario mapfrepe en todo el historial?",
        "qué rol tiene el usuario mapfrepe?",
        "pushes bloqueados del usuario mapfrepe este mes",
        "pushes sin ticket del usuario mapfrepe",
        "como conecto github a webhooks?",
        "que es un webhook?",
        "en que me puedes ayudar?",
    ],
    "mixed": [
        "¿Quién hizo push a main esta semana sin ticket de Jira?",
        "qué rol tiene el usuario mapfrepe?",
        "¿GitGov es open source?",
        "¿Qué plataformas soporta GitGov Desktop?",
        "¿cómo actualizo la app desktop?",
        "explica la diferencia entre auditoría y observabilidad en GitGov",
        "dame una guía para onboarding de org",
        "¿cómo funciona outbox offline?",
        "¿qué pasa si el servidor se cae?",
        "¿cómo se conecta jira?",
        "¿cómo se conecta jenkins?",
        "¿como conecto github a webhooks?",
    ],
    "llm_forced": [
        "Escribe una metáfora original sobre gobernanza git y océanos.",
        "Compara GitGov con un aeropuerto usando analogías técnicas.",
        "Redacta una explicación breve de políticas de repos con tono didáctico.",
        "Dame 3 ejemplos creativos para explicar branch protection a juniors.",
        "Explica por qué observabilidad y compliance no son lo mismo en 5 líneas.",
    ],
}


@dataclass
class Result:
    idx: int
    question: str
    http_code: int
    chat_status: str
    latency_ms: float
    ok: bool
    error_kind: Optional[str] = None
    error_message: Optional[str] = None


def percentile(values: List[float], p: float) -> float:
    if not values:
        return 0.0
    if len(values) == 1:
        return float(values[0])
    sorted_vals = sorted(values)
    pos = (len(sorted_vals) - 1) * (p / 100.0)
    lower = int(pos)
    upper = min(lower + 1, len(sorted_vals) - 1)
    weight = pos - lower
    return sorted_vals[lower] * (1 - weight) + sorted_vals[upper] * weight


def load_questions(args: argparse.Namespace) -> List[str]:
    if args.question_file:
        path = Path(args.question_file)
        if not path.exists():
            raise FileNotFoundError(f"Question file not found: {path}")
        lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]
        questions = [line for line in lines if line and not line.startswith("#")]
        if not questions:
            raise ValueError("Question file has no valid lines.")
        return questions

    scenario = args.scenario.lower()
    if scenario not in DEFAULT_SCENARIOS:
        raise ValueError(
            f"Unknown scenario '{args.scenario}'. Available: {', '.join(DEFAULT_SCENARIOS.keys())}"
        )
    return DEFAULT_SCENARIOS[scenario]


def chat_once(
    *,
    idx: int,
    question: str,
    server_url: str,
    api_key: str,
    timeout_sec: float,
    org_name: Optional[str],
) -> Result:
    url = server_url.rstrip("/") + "/chat/ask"
    payload: Dict[str, Any] = {"question": question, "org_name": org_name}
    data = json.dumps(payload).encode("utf-8")

    req = request.Request(url=url, method="POST", data=data)
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", f"Bearer {api_key}")

    started = time.perf_counter()
    try:
        with request.urlopen(req, timeout=timeout_sec) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            elapsed_ms = (time.perf_counter() - started) * 1000.0
            http_code = int(getattr(resp, "status", 0) or 0)
            chat_status = "unknown"
            try:
                parsed = json.loads(body)
                chat_status = str(parsed.get("status", "unknown"))
            except Exception:
                chat_status = "invalid_json"
            return Result(
                idx=idx,
                question=question,
                http_code=http_code,
                chat_status=chat_status,
                latency_ms=elapsed_ms,
                ok=http_code == 200,
            )
    except error.HTTPError as e:
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        return Result(
            idx=idx,
            question=question,
            http_code=int(e.code),
            chat_status="http_error",
            latency_ms=elapsed_ms,
            ok=False,
            error_kind="http_error",
            error_message=str(e),
        )
    except Exception as e:
        elapsed_ms = (time.perf_counter() - started) * 1000.0
        kind = "timeout" if "timed out" in str(e).lower() else "network_error"
        return Result(
            idx=idx,
            question=question,
            http_code=0,
            chat_status=kind,
            latency_ms=elapsed_ms,
            ok=False,
            error_kind=kind,
            error_message=str(e),
        )


def aggregate(results: List[Result], started: float, ended: float) -> Dict[str, Any]:
    total = len(results)
    duration_sec = max(ended - started, 1e-9)
    latencies = [r.latency_ms for r in results]
    ok_latencies = [r.latency_ms for r in results if r.ok]

    http_counts: Dict[str, int] = {}
    chat_status_counts: Dict[str, int] = {}
    error_counts: Dict[str, int] = {}
    for r in results:
        http_counts[str(r.http_code)] = http_counts.get(str(r.http_code), 0) + 1
        chat_status_counts[r.chat_status] = chat_status_counts.get(r.chat_status, 0) + 1
        if r.error_kind:
            error_counts[r.error_kind] = error_counts.get(r.error_kind, 0) + 1

    summary = {
        "total_requests": total,
        "duration_sec": duration_sec,
        "throughput_rps": total / duration_sec,
        "http_counts": http_counts,
        "chat_status_counts": chat_status_counts,
        "error_counts": error_counts,
        "latency_ms": {
            "min": min(latencies) if latencies else 0.0,
            "max": max(latencies) if latencies else 0.0,
            "avg": statistics.mean(latencies) if latencies else 0.0,
            "p50": percentile(latencies, 50),
            "p95": percentile(latencies, 95),
            "p99": percentile(latencies, 99),
            "ok_p50": percentile(ok_latencies, 50),
            "ok_p95": percentile(ok_latencies, 95),
            "ok_p99": percentile(ok_latencies, 99),
        },
    }
    return summary


def print_summary(summary: Dict[str, Any]) -> None:
    latency = summary["latency_ms"]
    print("")
    print("=== Chat Capacity Summary ===")
    print(f"total_requests   : {summary['total_requests']}")
    print(f"duration_sec     : {summary['duration_sec']:.2f}")
    print(f"throughput_rps   : {summary['throughput_rps']:.2f}")
    print("")
    print("HTTP counts:")
    for code, count in sorted(summary["http_counts"].items(), key=lambda x: x[0]):
        print(f"  {code:>4}: {count}")
    print("")
    print("Chat status counts:")
    for status, count in sorted(summary["chat_status_counts"].items(), key=lambda x: x[0]):
        print(f"  {status:>24}: {count}")
    if summary["error_counts"]:
        print("")
        print("Error kinds:")
        for kind, count in sorted(summary["error_counts"].items(), key=lambda x: x[0]):
            print(f"  {kind:>24}: {count}")
    print("")
    print("Latency ms (all):")
    print(
        f"  min={latency['min']:.1f} avg={latency['avg']:.1f} "
        f"p50={latency['p50']:.1f} p95={latency['p95']:.1f} p99={latency['p99']:.1f} max={latency['max']:.1f}"
    )
    print("Latency ms (HTTP 200 only):")
    print(
        f"  p50={latency['ok_p50']:.1f} p95={latency['ok_p95']:.1f} p99={latency['ok_p99']:.1f}"
    )
    print("")


def main() -> int:
    parser = argparse.ArgumentParser(description="GitGov chat capacity benchmark")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--api-key", default="")
    parser.add_argument("--org-name", default=None)
    parser.add_argument("--requests", type=int, default=120)
    parser.add_argument("--concurrency", type=int, default=12)
    parser.add_argument("--timeout-sec", type=float, default=25.0)
    parser.add_argument(
        "--scenario",
        default="mixed",
        choices=sorted(DEFAULT_SCENARIOS.keys()),
        help="Built-in question mix",
    )
    parser.add_argument("--question-file", default="")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--out-json", default="")
    args = parser.parse_args()

    api_key = args.api_key or ""
    if not api_key:
        api_key = (Path.cwd().parent / ".env").read_text(encoding="utf-8").split("VITE_API_KEY=")[-1].splitlines()[0].strip() if (Path.cwd().parent / ".env").exists() and "VITE_API_KEY=" in (Path.cwd().parent / ".env").read_text(encoding="utf-8") else ""
    if not api_key:
        print("ERROR: missing API key. Use --api-key or set VITE_API_KEY in gitgov/.env", file=sys.stderr)
        return 2

    questions = load_questions(args)
    random.seed(args.seed)

    run_set = [random.choice(questions) for _ in range(args.requests)]
    print("Running chat benchmark...")
    print(f"  server_url   : {args.server_url}")
    print(f"  requests     : {args.requests}")
    print(f"  concurrency  : {args.concurrency}")
    print(f"  timeout_sec  : {args.timeout_sec}")
    print(f"  scenario     : {args.scenario if not args.question_file else 'from_file'}")
    print("")

    started = time.perf_counter()
    results: List[Result] = []
    with ThreadPoolExecutor(max_workers=args.concurrency) as pool:
        futures = [
            pool.submit(
                chat_once,
                idx=i,
                question=run_set[i],
                server_url=args.server_url,
                api_key=api_key,
                timeout_sec=args.timeout_sec,
                org_name=args.org_name,
            )
            for i in range(args.requests)
        ]
        for fut in as_completed(futures):
            results.append(fut.result())
    ended = time.perf_counter()

    results.sort(key=lambda r: r.idx)
    summary = aggregate(results, started, ended)
    print_summary(summary)

    if args.out_json:
        out_path = Path(args.out_json)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out = {
            "config": {
                "server_url": args.server_url,
                "requests": args.requests,
                "concurrency": args.concurrency,
                "timeout_sec": args.timeout_sec,
                "scenario": args.scenario if not args.question_file else "from_file",
            },
            "summary": summary,
            "samples": [
                {
                    "idx": r.idx,
                    "question": r.question,
                    "http_code": r.http_code,
                    "chat_status": r.chat_status,
                    "latency_ms": round(r.latency_ms, 2),
                    "error_kind": r.error_kind,
                }
                for r in results[:50]
            ],
        }
        out_path.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"JSON report written to: {out_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

