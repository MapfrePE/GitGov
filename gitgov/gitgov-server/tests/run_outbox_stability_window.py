#!/usr/bin/env python3
"""
Run periodic outbox rollout guard checks over a time window (e.g., 24h).

Example:
  python tests/run_outbox_stability_window.py \
    --server-url http://127.0.0.1:3000 \
    --phase 100 \
    --strict \
    --duration-hours 24 \
    --interval-secs 3600 \
    --out-jsonl tests/artifacts/outbox_stability_24h.jsonl
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List


def run_guard(
    *,
    server_url: str,
    phase: int,
    strict: bool,
    api_key: str,
) -> Dict[str, Any]:
    cmd = [
        sys.executable,
        "tests/outbox_rollout_guard.py",
        "--server-url",
        server_url,
        "--phase",
        str(phase),
    ]
    if strict:
        cmd.append("--strict")
    if api_key.strip():
        cmd.extend(["--api-key", api_key.strip()])

    started_ms = int(time.time() * 1000)
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    ended_ms = int(time.time() * 1000)

    output = proc.stdout.strip()
    status_line = ""
    payload: Dict[str, Any] = {}
    for line in output.splitlines():
        if line.startswith("ROLLOUT_GUARD="):
            status_line = line
            break

    # Parse the first JSON object in stdout (guard prints one summary object).
    if "{" in output and "}" in output:
        start = output.find("{")
        end = output.rfind("}") + 1
        raw_json = output[start:end]
        try:
            payload = json.loads(raw_json)
        except json.JSONDecodeError:
            payload = {"parse_error": "failed_to_parse_guard_json"}

    return {
        "timestamp_ms": started_ms,
        "duration_ms": max(0, ended_ms - started_ms),
        "exit_code": proc.returncode,
        "status": status_line or "ROLLOUT_GUARD=UNKNOWN",
        "summary": payload,
        "stderr": proc.stderr.strip(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Periodic stability checks using outbox rollout guard")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--api-key", default="")
    parser.add_argument("--phase", type=int, choices=[10, 50, 100], default=100)
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--duration-hours", type=float, default=24.0)
    parser.add_argument("--interval-secs", type=int, default=3600)
    parser.add_argument("--out-jsonl", default="tests/artifacts/outbox_stability_24h.jsonl")
    args = parser.parse_args()

    duration_secs = max(1, int(args.duration_hours * 3600))
    interval_secs = max(1, args.interval_secs)
    out_path = Path(args.out_jsonl).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    started = time.time()
    deadline = started + duration_secs
    samples: List[Dict[str, Any]] = []
    failures = 0

    print("Outbox stability window started")
    print(f"  server_url    : {args.server_url}")
    print(f"  phase         : {args.phase}")
    print(f"  strict        : {args.strict}")
    print(f"  duration_secs : {duration_secs}")
    print(f"  interval_secs : {interval_secs}")
    print(f"  out_jsonl     : {out_path}")
    print("")

    while time.time() <= deadline:
        sample = run_guard(
            server_url=args.server_url,
            phase=args.phase,
            strict=args.strict,
            api_key=args.api_key,
        )
        samples.append(sample)
        if sample["exit_code"] != 0:
            failures += 1

        with out_path.open("a", encoding="utf-8") as f:
            f.write(json.dumps(sample, ensure_ascii=True) + "\n")

        print(
            f"sample={len(samples)} status={sample['status']} exit={sample['exit_code']} duration_ms={sample['duration_ms']}"
        )

        next_tick = time.time() + interval_secs
        if next_tick > deadline:
            break
        time.sleep(interval_secs)

    report = {
        "samples": len(samples),
        "failures": failures,
        "passed": failures == 0,
        "out_jsonl": str(out_path),
        "duration_secs": int(time.time() - started),
    }
    print("")
    print(json.dumps(report, indent=2))
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
