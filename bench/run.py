#!/usr/bin/env python3
"""
Warm-call latency harness for `mcp-server-git-rs` and the Python
`mcp-server-git` reference server.

For each named tool, opens one stdio MCP session per server, performs
the `initialize` handshake, then issues N `tools/call` requests in a
single long-running session and records the round-trip latency of each
call. Reports p50/p95 in milliseconds.

The pair of servers is compared on identical inputs against the same
fixture repository. Cold-start is measured separately (`hyperfine`); see
`bench/README.md`.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class ServerSpec:
    label: str
    cmd: list[str]


def jsonrpc_send(proc: subprocess.Popen, payload: dict[str, Any]) -> None:
    line = json.dumps(payload) + "\n"
    assert proc.stdin is not None
    proc.stdin.write(line.encode())
    proc.stdin.flush()


def jsonrpc_read(proc: subprocess.Popen) -> dict[str, Any]:
    assert proc.stdout is not None
    line = proc.stdout.readline()
    if not line:
        raise RuntimeError("server closed stdout")
    return json.loads(line)


def initialize(proc: subprocess.Popen) -> None:
    jsonrpc_send(
        proc,
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "bench", "version": "0"},
            },
        },
    )
    jsonrpc_read(proc)
    jsonrpc_send(proc, {"jsonrpc": "2.0", "method": "notifications/initialized"})


def call_tool(
    proc: subprocess.Popen,
    rpc_id: int,
    name: str,
    arguments: dict[str, Any],
) -> tuple[float, dict[str, Any]]:
    payload = {
        "jsonrpc": "2.0",
        "id": rpc_id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    }
    t0 = time.perf_counter_ns()
    jsonrpc_send(proc, payload)
    resp = jsonrpc_read(proc)
    elapsed_ms = (time.perf_counter_ns() - t0) / 1e6
    return elapsed_ms, resp


def run_one(
    server: ServerSpec,
    fixture: Path,
    tools: list[tuple[str, dict[str, Any]]],
    iterations: int,
    warmup: int,
) -> dict[str, dict[str, float]]:
    env = dict(os.environ)
    proc = subprocess.Popen(
        server.cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=env,
    )
    try:
        initialize(proc)
        rpc_id = 100
        results: dict[str, dict[str, float]] = {}
        for name, args in tools:
            for _ in range(warmup):
                rpc_id += 1
                _, resp = call_tool(proc, rpc_id, name, args)
                if "error" in resp:
                    raise RuntimeError(
                        f"{server.label} {name} errored: {resp['error']}"
                    )
            samples: list[float] = []
            for _ in range(iterations):
                rpc_id += 1
                ms, resp = call_tool(proc, rpc_id, name, args)
                if "error" in resp:
                    raise RuntimeError(
                        f"{server.label} {name} errored: {resp['error']}"
                    )
                samples.append(ms)
            samples.sort()
            results[name] = {
                "p50": statistics.median(samples),
                "p95": samples[max(0, int(len(samples) * 0.95) - 1)],
                "mean": statistics.fmean(samples),
                "n": len(samples),
            }
        return results
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()


def cold_start(server: ServerSpec, runs: int = 30) -> dict[str, float]:
    """Measure server start → initialize-response time (no warm pool)."""
    samples: list[float] = []
    for _ in range(runs):
        proc = subprocess.Popen(
            server.cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        try:
            t0 = time.perf_counter_ns()
            jsonrpc_send(
                proc,
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {},
                        "clientInfo": {"name": "bench", "version": "0"},
                    },
                },
            )
            jsonrpc_read(proc)
            samples.append((time.perf_counter_ns() - t0) / 1e6)
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
    samples.sort()
    return {
        "p50": statistics.median(samples),
        "p95": samples[max(0, int(len(samples) * 0.95) - 1)],
        "mean": statistics.fmean(samples),
        "n": len(samples),
    }


def fmt_ms(x: float) -> str:
    return f"{x:.2f}"


def disclosure() -> str:
    uname = platform.uname()
    cpu = "unknown"
    if sys.platform == "darwin":
        try:
            cpu = subprocess.check_output(
                ["sysctl", "-n", "machdep.cpu.brand_string"], text=True
            ).strip()
        except Exception:
            pass
    elif sys.platform.startswith("linux"):
        try:
            with open("/proc/cpuinfo") as f:
                for line in f:
                    if "model name" in line:
                        cpu = line.split(":", 1)[1].strip()
                        break
        except Exception:
            pass

    rust_ver = "unknown"
    try:
        rust_ver = subprocess.check_output(
            ["rustc", "--version"], text=True
        ).strip()
    except Exception:
        pass

    py_ver = sys.version.replace("\n", " ")
    return (
        f"- **CPU**: {cpu}\n"
        f"- **OS**: {uname.system} {uname.release} ({uname.machine})\n"
        f"- **Rust**: {rust_ver}\n"
        f"- **Python**: {py_ver}\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--rust-bin",
        default=str(
            Path(__file__).resolve().parents[1]
            / "target"
            / "release"
            / "mcp-server-git-rs"
        ),
        help="Path to the release mcp-server-git-rs binary.",
    )
    parser.add_argument(
        "--python-cmd",
        nargs="+",
        default=["uvx", "mcp-server-git"],
        help="Command (with args) that launches the Python comparator.",
    )
    parser.add_argument(
        "--fixture",
        default=str(Path(__file__).resolve().parents[1]),
        help="Path to the fixture git repository (default: this repo).",
    )
    parser.add_argument("--iterations", type=int, default=200)
    parser.add_argument("--warmup", type=int, default=20)
    parser.add_argument("--cold-runs", type=int, default=30)
    parser.add_argument(
        "--skip-python",
        action="store_true",
        help="Only measure the Rust server (e.g. when uvx is unavailable).",
    )
    parser.add_argument(
        "--out",
        default=None,
        help="Optional path to write a Markdown report.",
    )
    args = parser.parse_args()

    fixture = Path(args.fixture).resolve()
    if not (fixture / ".git").exists():
        print(f"fixture not a git repo: {fixture}", file=sys.stderr)
        return 2
    if not Path(args.rust_bin).exists():
        print(
            f"rust binary missing: {args.rust_bin}\n"
            f"build it first: cargo build --release",
            file=sys.stderr,
        )
        return 2

    rust = ServerSpec(
        label="mcp-server-git-rs",
        cmd=[args.rust_bin, "-r", str(fixture)],
    )
    python = ServerSpec(
        label="mcp-server-git (python)",
        cmd=[*args.python_cmd, "-r", str(fixture)],
    )

    repo_arg = {"repo_path": str(fixture)}
    head_rev = (
        subprocess.check_output(
            ["git", "-C", str(fixture), "rev-parse", "HEAD"], text=True
        )
        .strip()
    )
    tools_to_test: list[tuple[str, dict[str, Any]]] = [
        ("git_status", repo_arg),
        ("git_log", repo_arg),
        # branch_type is required by the Python server's schema (Rust defaults
        # to "local"); pass it explicitly so both servers do equivalent work.
        ("git_branch", {**repo_arg, "branch_type": "local"}),
        ("git_diff_unstaged", repo_arg),
        ("git_diff_staged", repo_arg),
        ("git_show", {**repo_arg, "revision": head_rev}),
    ]

    print("Cold start (server spawn → initialize response):")
    cold_rust = cold_start(rust, runs=args.cold_runs)
    print(
        f"  {rust.label:30s} p50={fmt_ms(cold_rust['p50'])} ms  "
        f"p95={fmt_ms(cold_rust['p95'])} ms  mean={fmt_ms(cold_rust['mean'])} ms  "
        f"n={cold_rust['n']}"
    )
    cold_py: dict[str, float] | None = None
    if not args.skip_python and shutil.which(args.python_cmd[0]):
        cold_py = cold_start(python, runs=args.cold_runs)
        print(
            f"  {python.label:30s} p50={fmt_ms(cold_py['p50'])} ms  "
            f"p95={fmt_ms(cold_py['p95'])} ms  mean={fmt_ms(cold_py['mean'])} ms  "
            f"n={cold_py['n']}"
        )

    print(
        f"\nWarm-call latency (one stdio session, {args.iterations} iterations "
        f"per tool, {args.warmup} warmup):"
    )
    rust_warm = run_one(rust, fixture, tools_to_test, args.iterations, args.warmup)
    py_warm: dict[str, dict[str, float]] | None = None
    if not args.skip_python and shutil.which(args.python_cmd[0]):
        py_warm = run_one(
            python, fixture, tools_to_test, args.iterations, args.warmup
        )

    header = (
        f"| Tool | rust p50 | rust p95 | python p50 | python p95 | speedup p50 |\n"
        f"|---|---:|---:|---:|---:|---:|\n"
    )
    rows = []
    for name, _ in tools_to_test:
        r = rust_warm[name]
        rust_p50 = fmt_ms(r["p50"])
        rust_p95 = fmt_ms(r["p95"])
        if py_warm is not None:
            p = py_warm[name]
            py_p50 = fmt_ms(p["p50"])
            py_p95 = fmt_ms(p["p95"])
            speedup = f"{p['p50'] / r['p50']:.1f}×"
        else:
            py_p50 = py_p95 = speedup = "—"
        rows.append(
            f"| `{name}` | {rust_p50} | {rust_p95} | {py_p50} | {py_p95} | {speedup} |"
        )
    table = header + "\n".join(rows)
    print()
    print(table)

    if args.out:
        cold_table_lines = [
            "| Server | p50 (ms) | p95 (ms) | mean (ms) | runs |",
            "|---|---:|---:|---:|---:|",
            f"| `mcp-server-git-rs` | {fmt_ms(cold_rust['p50'])} | "
            f"{fmt_ms(cold_rust['p95'])} | {fmt_ms(cold_rust['mean'])} | "
            f"{cold_rust['n']} |",
        ]
        if cold_py is not None:
            cold_table_lines.append(
                f"| `mcp-server-git` (python via uvx) | "
                f"{fmt_ms(cold_py['p50'])} | {fmt_ms(cold_py['p95'])} | "
                f"{fmt_ms(cold_py['mean'])} | {cold_py['n']} |"
            )
        cold_table = "\n".join(cold_table_lines)
        report = (
            f"# Bench results — {time.strftime('%Y-%m-%d')}\n\n"
            f"Fixture: `{fixture}` "
            f"(HEAD `{head_rev[:12]}`)\n\n"
            f"{disclosure()}\n"
            f"## Cold start (spawn → initialize response)\n\n"
            f"{cold_table}\n\n"
            f"## Warm-call latency\n\n"
            f"One stdio session per server. {args.iterations} iterations per tool "
            f"after {args.warmup} warmup calls.\n\n"
            f"{table}\n\n"
            f"Reproduce: `cd bench && python3 run.py --out results/$(date +%Y-%m-%d).md`\n"
        )
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(report)
        print(f"\nwrote {args.out}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
