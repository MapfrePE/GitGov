#!/usr/bin/env python3
"""
Pre-prod forensic scan:
1) Secret exposure scan (working tree + recent git history)
2) Incident marker scan in local test/runtime logs
3) Runtime contract checks (/health, /stats, /logs, dedup on /events)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from urllib import error, request


ROOT = Path(__file__).resolve().parents[2]
SERVER_ROOT = Path(__file__).resolve().parents[1]


PATTERNS: Dict[str, re.Pattern[str]] = {
    "github_pat_token": re.compile(r"\bghp_[A-Za-z0-9]{20,}\b"),
    "google_api_key": re.compile(r"\bAIza[0-9A-Za-z_-]{20,}\b"),
    "openai_like_key": re.compile(r"\bsk-[A-Za-z0-9]{20,}\b"),
    "gemini_env_assignment": re.compile(r"^\s*GEMINI_API_KEY\s*=", re.IGNORECASE),
    "github_pat_assignment": re.compile(
        r"^\s*GITHUB_PERSONAL_ACCESS_TOKEN\s*=", re.IGNORECASE
    ),
    "supabase_service_assignment": re.compile(
        r"^\s*SUPABASE_SERVICE_KEY\s*=", re.IGNORECASE
    ),
    "supabase_anon_assignment": re.compile(r"^\s*SUPABASE_ANON_KEY\s*=", re.IGNORECASE),
}

HISTORY_GREP_REGEX = (
    r"ghp_[A-Za-z0-9]{20,}|AIza[0-9A-Za-z_-]{20,}|sk-[A-Za-z0-9]{20,}|"
    r"GEMINI_API_KEY|GITHUB_PERSONAL_ACCESS_TOKEN|SUPABASE_SERVICE_KEY|SUPABASE_ANON_KEY"
)

LOG_MARKERS = {
    "max_clients_session_mode": re.compile(r"MaxClientsInSessionMode"),
    "db_error_fail_open": re.compile(r"db_error_fail_open", re.IGNORECASE),
    "unexpected_401": re.compile(r"\b401\b|Unauthorized", re.IGNORECASE),
    "auth_backend_unavailable": re.compile(r"Authentication backend unavailable", re.IGNORECASE),
}

SKIP_DIRS = {
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "__pycache__",
}

SKIP_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".pdf",
    ".zip",
    ".exe",
    ".dll",
    ".bin",
}


@dataclass
class Finding:
    kind: str
    file: str
    line: int
    source: str
    commit: Optional[str] = None


def should_skip_path(path: Path) -> bool:
    parts = set(path.parts)
    if parts.intersection(SKIP_DIRS):
        return True
    if path.suffix.lower() in SKIP_SUFFIXES:
        return True
    return False


def read_api_key() -> str:
    env_file = SERVER_ROOT / ".env"
    if not env_file.exists():
        raise RuntimeError(f"Missing {env_file}")
    for raw in env_file.read_text(encoding="utf-8", errors="ignore").splitlines():
        line = raw.strip()
        if line.startswith("GITGOV_API_KEY="):
            value = line.split("=", 1)[1].strip().strip('"').strip("'")
            if value:
                return value
    raise RuntimeError("GITGOV_API_KEY not found in gitgov-server/.env")


def scan_working_tree() -> List[Finding]:
    findings: List[Finding] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or should_skip_path(path):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        rel = path.relative_to(ROOT).as_posix()
        for idx, line in enumerate(text.splitlines(), start=1):
            for kind, pattern in PATTERNS.items():
                if pattern.search(line):
                    findings.append(
                        Finding(
                            kind=kind,
                            file=rel,
                            line=idx,
                            source="working_tree",
                        )
                    )
    return findings


def scan_recent_history(max_commits: int) -> List[Finding]:
    findings: List[Finding] = []
    rev_list = subprocess.run(
        ["git", "rev-list", f"--max-count={max_commits}", "HEAD"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        check=False,
    )
    commits = [c.strip() for c in rev_list.stdout.splitlines() if c.strip()]
    seen = set()

    for commit in commits:
        grep = subprocess.run(
            [
                "git",
                "grep",
                "-I",
                "-n",
                "-E",
                HISTORY_GREP_REGEX,
                commit,
                "--",
                ".",
            ],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            check=False,
        )
        if grep.returncode not in (0, 1):
            continue
        for raw in grep.stdout.splitlines():
            # Expected format: <commit>:<path>:<line>:<content>
            parts = raw.split(":", 3)
            if len(parts) < 4:
                continue
            hit_commit, hit_path, hit_line, content = parts
            try:
                line_no = int(hit_line)
            except ValueError:
                continue

            kind = "history_secret_marker"
            for pattern_name, pattern in PATTERNS.items():
                if pattern.search(content):
                    kind = pattern_name
                    break

            key = (hit_commit, hit_path, line_no, kind)
            if key in seen:
                continue
            seen.add(key)
            findings.append(
                Finding(
                    kind=kind,
                    file=hit_path,
                    line=line_no,
                    source="git_history",
                    commit=hit_commit,
                )
            )
    return findings


def scan_logs() -> Dict[str, Any]:
    tests_dir = SERVER_ROOT / "tests"
    files = sorted(
        [p for p in tests_dir.glob("*.log") if p.is_file()],
        key=lambda p: p.name.lower(),
    )
    marker_counts: Dict[str, int] = {k: 0 for k in LOG_MARKERS}
    files_with_hits: Dict[str, List[str]] = {k: [] for k in LOG_MARKERS}

    for path in files:
        text = path.read_text(encoding="utf-8", errors="ignore")
        for marker_name, marker_re in LOG_MARKERS.items():
            count = len(marker_re.findall(text))
            if count > 0:
                marker_counts[marker_name] += count
                files_with_hits[marker_name].append(path.name)

    return {
        "log_files_scanned": len(files),
        "marker_counts": marker_counts,
        "files_with_hits": files_with_hits,
    }


def http_json(
    *,
    base_url: str,
    api_key: str,
    endpoint: str,
    method: str = "GET",
    payload: Optional[Dict[str, Any]] = None,
    timeout_sec: float = 15.0,
) -> Tuple[int, Dict[str, Any]]:
    url = base_url.rstrip("/") + endpoint
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    req = request.Request(url=url, method=method, data=data)
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")

    try:
        with request.urlopen(req, timeout=timeout_sec) as resp:
            body = resp.read().decode("utf-8")
            return int(resp.status), (json.loads(body) if body else {})
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


def runtime_checks(base_url: str, api_key: str) -> Dict[str, Any]:
    report: Dict[str, Any] = {}

    health_code, health_body = http_json(
        base_url=base_url, api_key=api_key, endpoint="/health", method="GET"
    )
    report["health_status"] = health_code
    report["health_ok"] = health_code == 200 and health_body.get("status") == "ok"

    stats_code, stats_body = http_json(base_url=base_url, api_key=api_key, endpoint="/stats")
    report["stats_status"] = stats_code
    report["stats_ok"] = stats_code == 200 and isinstance(stats_body, dict)

    logs_code, logs_body = http_json(
        base_url=base_url, api_key=api_key, endpoint="/logs?limit=5&offset=0"
    )
    report["logs_status"] = logs_code
    report["logs_ok"] = (
        logs_code == 200
        and isinstance(logs_body, dict)
        and isinstance(logs_body.get("events", []), list)
        and logs_body.get("error") is None
    )

    event_uuid = str(uuid.uuid4())
    event_payload = {
        "events": [
            {
                "event_uuid": event_uuid,
                "event_type": "commit",
                "user_login": "forensic_preprod",
                "repo_full_name": "MapfrePE/GitGov",
                "files": [],
                "status": "success",
                "timestamp": int(time.time() * 1000),
            }
        ],
        "client_version": "forensic-preprod",
    }
    first_code, first_body = http_json(
        base_url=base_url,
        api_key=api_key,
        endpoint="/events",
        method="POST",
        payload=event_payload,
    )
    second_code, second_body = http_json(
        base_url=base_url,
        api_key=api_key,
        endpoint="/events",
        method="POST",
        payload=event_payload,
    )
    report["events_first_status"] = first_code
    report["events_second_status"] = second_code
    report["dedup_first_accepted"] = event_uuid in first_body.get("accepted", [])
    report["dedup_second_duplicate"] = event_uuid in second_body.get("duplicates", [])
    report["dedup_ok"] = report["dedup_first_accepted"] and report["dedup_second_duplicate"]

    report["runtime_ok"] = (
        report["health_ok"] and report["stats_ok"] and report["logs_ok"] and report["dedup_ok"]
    )
    return report


def summarize_findings(findings: List[Finding]) -> Dict[str, Any]:
    by_kind: Dict[str, int] = {}
    for f in findings:
        by_kind[f.kind] = by_kind.get(f.kind, 0) + 1
    return {
        "total_findings": len(findings),
        "by_kind": by_kind,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run pre-prod forensic scan")
    parser.add_argument("--server-url", default="http://127.0.0.1:3000")
    parser.add_argument("--max-history-commits", type=int, default=50)
    parser.add_argument("--skip-runtime", action="store_true")
    parser.add_argument(
        "--out-json",
        default=f"tests/artifacts/forensic_preprod_scan_{time.strftime('%Y-%m-%d')}.json",
    )
    args = parser.parse_args()

    working = scan_working_tree()
    history = scan_recent_history(args.max_history_commits)
    log_summary = scan_logs()

    runtime: Dict[str, Any] = {"skipped": args.skip_runtime}
    exit_code = 0

    if not args.skip_runtime:
        api_key = read_api_key()
        runtime = runtime_checks(args.server_url, api_key)
        if not runtime.get("runtime_ok", False):
            exit_code = 1

    report = {
        "timestamp_ms": int(time.time() * 1000),
        "server_url": args.server_url,
        "working_tree": summarize_findings(working),
        "git_history": {
            "max_commits_scanned": args.max_history_commits,
            **summarize_findings(history),
        },
        "logs": log_summary,
        "runtime": runtime,
        "samples": {
            "working_tree_first_50": [
                f.__dict__ for f in working[:50]
            ],
            "git_history_first_50": [
                f.__dict__ for f in history[:50]
            ],
        },
    }

    out_path = (SERVER_ROOT / args.out_json).resolve() if not Path(args.out_json).is_absolute() else Path(args.out_json)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print(json.dumps(report, indent=2))
    print(f"artifact={out_path}")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
