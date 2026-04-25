# Benchmarks

Reproducible harness for `mcp-server-git-rs` against the Python
[`mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
reference server.

## What we measure

Two distinct things — they have different shapes and different agent-loop
implications.

1. **Cold start** — time from `Popen(...)` until the server's response to a
   single `initialize` request. This is the latency a host like Claude Desktop
   sees on every fresh launch.
2. **Warm-call latency** — round-trip time of `tools/call` over a long-running
   stdio session that already finished `initialize`. Reported as p50 / p95 in
   milliseconds across N iterations per tool. This is the per-call cost an
   agent loop pays once it has connected.

A note on cold-start tails: the very first spawn on a freshly-booted machine
can be several times the steady-state p50 because dyld and the page cache
are cold. We report `mean` alongside `p50/p95` so a single tail is visible;
if your first reproduction shows an outsized mean, run a second pass.

## What we don't claim

- This isn't libgit2 vs. shell-`git`. We measure the **MCP layer overhead**
  each implementation adds on top of its underlying git library — process
  startup, JSON-RPC framing, parameter validation, and the round-trip from
  `tools/call` arrival to `CallToolResult`.
- Numbers reflect one machine and one fixture repository. Run the harness on
  your own setup before quoting figures.

## Tools compared

Both servers must expose identical tool names + schemas to be a fair
comparison. We measure the read-only tools that overlap:

- `git_status`
- `git_log`
- `git_branch`
- `git_diff_unstaged`
- `git_diff_staged`
- `git_show` (called with `revision = HEAD`)

Write-side tools (`git_commit`, `git_add`, `git_reset`, `git_checkout`) are
excluded because they would mutate the fixture between calls and skew p95
samples toward the first iteration.

`git_branch` is called with `branch_type: "local"` explicitly: the Python
server's schema requires the field while Rust defaults to `"local"`. Without
the explicit argument, Python returns a fast schema-validation error and
the comparison is meaningless.

## How to reproduce

Requirements: a release build of this server, Python 3.11+, `uvx` (for the
Python comparator), and `hyperfine` if you also want cold-start numbers via a
second tool.

```bash
# from the repo root
cargo build --release
cd bench

# default: 200 warm iterations / 30 cold runs per server, this repo as fixture
python3 run.py --out results/$(date +%Y-%m-%d).md

# rust only, e.g. on a machine without uvx
python3 run.py --skip-python --out results/$(date +%Y-%m-%d)-rust-only.md

# different fixture (must be a git repo)
python3 run.py --fixture /path/to/some/repo --out results/large.md
```

`run.py` spawns each server as a subprocess, performs the MCP handshake,
issues `tools/call` requests over stdio, and times each round trip with
`time.perf_counter_ns()`. p50 and p95 are computed per tool per server.

## Disclosure expected on every result

Every captured result must include:

- CPU model
- OS + kernel version
- `rustc --version`
- `mcp-server-git-rs --version`
- `uvx --version` and Python interpreter version (if comparing)
- Fixture path + `git rev-parse HEAD` (so the object graph is reproducible)

`run.py --out <file>` includes most of this automatically.

## Captured runs

See [`results/`](./results) for runs committed to the repo. Each file is a
self-contained Markdown report with disclosure header + tables.
