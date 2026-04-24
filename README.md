# mcp-server-git-rs

A fast Rust MCP server that exposes git repository operations over stdio. Drop-in
replacement for the Python [`mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
reference server: same tool names, same input schemas, same stdio transport.

Single static binary, ~10 ms cold start, ~60× faster than the Python server on a
Claude-Desktop-shaped `initialize` + `tools/list` handshake.

## What it does

Wraps [libgit2](https://libgit2.org/) (via [`git2`](https://crates.io/crates/git2))
behind [rmcp](https://crates.io/crates/rmcp), the official Rust MCP SDK. Every
tool call runs in-process — no shelling out to `git`, no Python interpreter, no
`uvx` environment resolution per call.

## Tools

| Tool | Description | Annotations |
|---|---|---|
| `git_status` | Working tree status | read-only |
| `git_diff_unstaged` | Unstaged changes | read-only |
| `git_diff_staged` | Staged changes | read-only |
| `git_diff` | Diff working tree against a branch/tag/commit | read-only |
| `git_commit` | Record staged changes | — |
| `git_add` | Stage files; `["."]` stages everything | — |
| `git_reset` | Unstage all | destructive |
| `git_log` | Commit log with optional date filtering | read-only |
| `git_create_branch` | Create a branch | — |
| `git_checkout` | Switch branches | — |
| `git_show` | Show a commit | read-only |
| `git_branch` | List branches (local/remote/all, with `contains`/`not_contains`) | read-only |
| `git_push` *(new)* | Push to a remote. SSH agent for `git@`, credential helper for `https://` | destructive |

`git_push` is the only tool the Python reference server does not expose. Everything
else is a one-to-one port.

## Installation

```bash
cargo install mcp-server-git-rs
```

Or build locally:

```bash
git clone <repo>
cd mcp-server-git-rs
cargo build --release
# binary: target/release/mcp-server-git-rs
```

## Usage

```
mcp-server-git-rs [OPTIONS]

  -r, --repository <PATH>   restrict operations to this repo (all tool calls
                            must resolve inside it after canonicalisation)
  -v, --verbose             repeatable: -v info, -vv debug (default: warn)
```

Logs go to stderr; stdout is the MCP channel.

### Claude Desktop

`claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "git": {
      "command": "mcp-server-git-rs",
      "args": ["-r", "/Users/you/code/my-repo"]
    }
  }
}
```

### VS Code / Zed / any MCP-speaking client

Same shape — invoke the binary over stdio; all arguments and tool schemas match
the Python server.

## `git_push` and authentication

`git_push` reuses the same credentials your shell's `git push` would:

| Remote | Auth source |
|---|---|
| `git@host:...` (SSH) | keys loaded in `ssh-agent` |
| `https://...`        | the configured git credential helper (Keychain / libsecret / GCM) |
| `https://...` with no helper | `$MCP_GIT_TOKEN`, if set (used as the password against user `x-access-token`) |

No SSH-key-file reading — agent only — so a headless server never prompts for a
passphrase. Force-push is opt-in via `"force": true`; `set_upstream` writes
`branch.<name>.remote` and `branch.<name>.merge` to local config.

Push success is detected via libgit2's `push_update_reference` callback, so
server-side rejections (fast-forward, hook rejection) surface as tool errors even
when the underlying transport reports OK.

## Security

All of the Python server's invariants are preserved:

- **Repo scoping.** Every `repo_path` is canonicalised and, if `--repository` is
  set, must live under the canonical allowed root. Blocks `..` traversal and
  symlink escapes.
- **Flag-injection rejection.** User-supplied refs, branch names, timestamps,
  and filter strings that start with `-` are rejected before reaching libgit2.
- **Defense-in-depth ref validation.** `git_diff` and `git_checkout` call
  `revparse_single` after the flag check; unknown refs surface as clean errors,
  not crashes.

## `git_log` timestamp parsing

Accepts ISO-8601 / RFC-3339 and a small set of relative forms via
[`jiff`](https://crates.io/crates/jiff):

| Form | Example |
|---|---|
| RFC-3339 / ISO timestamp | `2024-01-15T14:30:25Z` |
| ISO date                 | `2024-01-15` |
| ISO local date-time      | `2024-01-15T14:30:25` |
| Relative span            | `2 weeks ago`, `3 days`, `1 month ago` |

**Not supported (unlike Python's `git log --since`):** free-form approxidate
phrases like `yesterday`, `last tuesday`, `noon`. The ISO and
`<N> <unit> [ago]` forms cover the realistic agent-loop cases; if there is demand
for the rest we can add an approxidate shim.

## Benchmark

Measured against the Python reference server on `initialize` + `tools/list`:

```
python:  679.3 ms ± 55.3 ms    (50 runs, warm)
rust:     11.4 ms ±  1.4 ms    (50 runs, warm)

rust ran 59.84 × ± 8.76 × faster than python
```

Inside a single long-running session (the shape an agent loop actually
produces), warm `git_status` over stdio averages **~1.7 ms/call** on an empty
test repo — throughput around 580 calls/second.

## Relationship to `mcp-server-git`

This is a port, not a fork. The canonical reference server remains the
[Python one](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
under the MCP project; run it with `uvx mcp-server-git`. This crate exists for
environments where Python startup is on the hot path (agent loops, CI jobs) and
a static binary is preferable.

Tool names, parameter names, default values, error shapes, and security
invariants match Python. A `claude_desktop_config.json` entry can be switched
between the two by changing only the `command`.

## License

MIT.
