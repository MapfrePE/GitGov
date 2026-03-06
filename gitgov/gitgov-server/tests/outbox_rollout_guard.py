#!/usr/bin/env python3
"""
Operational guard for gradual outbox coordination rollout.

Usage examples:
  python tests/outbox_rollout_guard.py --server-url http://127.0.0.1:3000 --phase 10
  python tests/outbox_rollout_guard.py --server-url http://127.0.0.1:3000 --phase 50 --api-key <key>
  python tests/outbox_rollout_guard.py --server-url http://127.0.0.1:3000 --phase 100 --strict
"""

from __future__ import annotations

import argparse
import json
import time
import uuid
from pathlib import Path
from typing import Any, Dict, Optional, Tuple
from urllib import error, request


def read_api_key(explicit: str) -> str:
    if explicit.strip():
        return explicit.strip()
    env_file = Path(__file__).resolve().parents[1] / ".env"
    if env_file.exists():
        for raw in env_file.read_text(encoding="utf-8").splitlines():
            line = raw.strip()
            if line.startswith("GITGOV_API_KEY="):
                value = line.split("=", 1)[1].strip().strip('"').strip("'")
                if value:
                    return value
    raise RuntimeError("Missing API key: pass --api-key or define GITGOV_API_KEY in gitgov-server/.env")


def http_json(
    *,
    server_url: str,
    api_key: str,
    endpoint: str,
    method: str = "GET",
    payload: Optional[Dict[str, Any]] = None,
    timeout_sec: float = 12.0,
) -> Tuple[int, Dict[str, Any]]:
    url = server_url.rstrip("/") + endpoint
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    req = request.Request(url=url, method=method, data=body)
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")
    try:
        with request.urlopen(req, timeout=timeout_sec) as resp:
            raw = resp.read().decode("utf-8")
            return int(resp.status), (json.loads(raw) if raw else {})
    except error.HTTPError as e:
        detail = ""
        try:
            detail = e.read().decode("utf-8")
        except Exception:  # noqa: BLE001
            detail = str(e)
        return int(e.code), {"error": detail}
    except error.URLError as e:
        reason = getattr(e, "reason", e)
        return 0, {"error": f"network_error: {reason}"}


def run_smoke(server_url: str, api_key: str) -> Dict[str, Any]:
    commit_uuid = str(uuid.uuid4())
    payload = {
        "events": [
            {
                "event_uuid": commit_uuid,
                "event_type": "commit",
                "user_login": "rollout_guard",
                "repo_full_name": "MapfrePE/GitGov",
                "files": [],
                "status": "success",
                "timestamp": int(time.time() * 1000),
            }
        ],
        "client_version": "outbox-rollout-guard",
    }

    events_code, events_body = http_json(
        server_url=server_url, api_key=api_key, endpoint="/events", method="POST", payload=payload
    )
    stats_code, stats_body = http_json(server_url=server_url, api_key=api_key, endpoint="/stats")
    logs_code, logs_body = http_json(
        server_url=server_url, api_key=api_key, endpoint="/logs?limit=5&offset=0"
    )

    return {
        "events_code": events_code,
        "events_accepted": len(events_body.get("accepted", [])),
        "stats_code": stats_code,
        "stats_has_client_events": "client_events" in stats_body,
        "logs_code": logs_code,
        "logs_error": logs_body.get("error"),
        "logs_events": len(logs_body.get("events", [])),
    }


def evaluate(
    phase: int,
    strict: bool,
    before: Dict[str, Any],
    after: Dict[str, Any],
    smoke: Dict[str, Any],
) -> Tuple[bool, Dict[str, Any]]:
    b = before.get("telemetry", {})
    a = after.get("telemetry", {})

    delta_total = int(a.get("total_requests", 0)) - int(b.get("total_requests", 0))
    delta_fail_open_db = int(a.get("fail_open_db_error_requests", 0)) - int(
        b.get("fail_open_db_error_requests", 0)
    )
    delta_denied = int(a.get("denied_requests", 0)) - int(b.get("denied_requests", 0))
    denied_ratio = (delta_denied / delta_total) if delta_total > 0 else 0.0
    fail_open_db_ratio = (delta_fail_open_db / delta_total) if delta_total > 0 else 0.0

    if strict:
        max_fail_open_db_ratio = 0.0
    elif phase <= 10:
        max_fail_open_db_ratio = 0.05
    elif phase <= 50:
        max_fail_open_db_ratio = 0.02
    else:
        max_fail_open_db_ratio = 0.01

    checks = {
        "smoke_events_ok": smoke["events_code"] == 200 and smoke["events_accepted"] >= 1,
        "smoke_stats_ok": smoke["stats_code"] == 200 and smoke["stats_has_client_events"],
        "smoke_logs_ok": smoke["logs_code"] == 200 and smoke["logs_error"] is None,
        "lease_metrics_recording": delta_total >= 0,
        "db_fail_open_within_threshold": fail_open_db_ratio <= max_fail_open_db_ratio,
    }
    ok = all(checks.values())
    summary = {
        "phase": phase,
        "strict": strict,
        "delta_total_requests": delta_total,
        "delta_denied_requests": delta_denied,
        "delta_fail_open_db_error_requests": delta_fail_open_db,
        "denied_ratio": denied_ratio,
        "fail_open_db_ratio": fail_open_db_ratio,
        "max_fail_open_db_ratio": max_fail_open_db_ratio,
        "checks": checks,
        "smoke": smoke,
    }
    return ok, summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Guardrail validation for outbox rollout phases")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--api-key", default="")
    parser.add_argument("--phase", type=int, choices=[10, 50, 100], required=True)
    parser.add_argument("--strict", action="store_true", help="require zero db_fail_open ratio")
    parser.add_argument("--out-json", default="")
    args = parser.parse_args()

    api_key = read_api_key(args.api_key)

    before_code, before = http_json(
        server_url=args.server_url, api_key=api_key, endpoint="/outbox/lease/metrics"
    )
    if before_code == 0:
        print("ROLLOUT_GUARD=FAIL")
        print(
            json.dumps(
                {
                    "error": "server_unreachable",
                    "server_url": args.server_url,
                    "hint": "Start gitgov-server and retry. Example: cd gitgov/gitgov-server && cargo run",
                    "details": before.get("error"),
                },
                indent=2,
            )
        )
        return 2
    smoke = run_smoke(args.server_url, api_key)
    after_code, after = http_json(
        server_url=args.server_url, api_key=api_key, endpoint="/outbox/lease/metrics"
    )

    if before_code != 200 or after_code != 200:
        print("ROLLOUT_GUARD=FAIL")
        print(f"metrics_before_code={before_code} metrics_after_code={after_code}")
        return 2

    ok, summary = evaluate(args.phase, args.strict, before, after, smoke)
    summary["server_url"] = args.server_url
    summary["timestamp_ms"] = int(time.time() * 1000)

    print("ROLLOUT_GUARD=PASS" if ok else "ROLLOUT_GUARD=FAIL")
    print(json.dumps(summary, indent=2))

    if args.out_json.strip():
        out_path = Path(args.out_json).resolve()
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
        print(f"artifact={out_path}")

    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
