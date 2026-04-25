# mcp-server-git-rs

[![Crates.io](https://img.shields.io/crates/v/mcp-server-git-rs.svg)](https://crates.io/crates/mcp-server-git-rs)
[![CI](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/ci.yml)
[![Release](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/release.yml/badge.svg)](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](Cargo.toml)

A fast Rust MCP server that exposes git repository operations over stdio. Drop-in
replacement for the Python [`mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
reference server: same tool names, same input schemas, same stdio transport.

Single static binary, ~10 ms cold start, ~60× faster than the Python server on a
Claude-Desktop-shaped `initialize` + `tools/list` handshake.

## Highlights

- **13 tools** — the full Python reference set plus `git_push` with ssh-agent /
  credential-helper auth.
- **~60× faster cold start** than `uvx mcp-server-git` on the handshake path.
- **Single static binary** (~4.7 MB stripped). No Python runtime, no `uvx`, no
  per-call environment resolution.
- **Security-hardened** — canonicalised repo scoping (`--repository`),
  flag-injection rejection, `revparse` validation. Matches the Python server's
  invariants one-to-one.
- **Works with 12+ MCP-speaking agents** — see [Use with your agent](#use-with-your-agent).

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

`git_push` is the only tool the Python reference server does not expose.
Everything else is a one-to-one port.

## Install

### Prebuilt binary (fastest)

Grab the tarball for your platform from the [latest release][releases]. Each
tarball extracts to a folder containing the binary, `LICENSE`, and `README.md`,
and ships with a `.sha256` sibling for verification.

```bash
# Pick the triple that matches your machine
TAG=v0.1.0
TRIPLE=aarch64-apple-darwin   # or x86_64-apple-darwin, x86_64-unknown-linux-gnu,
                              #    aarch64-unknown-linux-gnu, x86_64-unknown-linux-musl,
                              #    aarch64-unknown-linux-musl

BASE="https://github.com/karan-vk/mcp-server-git-rs/releases/download/$TAG"
NAME="mcp-server-git-rs-$TAG-$TRIPLE"

curl -LO "$BASE/$NAME.tar.gz"
curl -LO "$BASE/$NAME.tar.gz.sha256"
shasum -a 256 -c "$NAME.tar.gz.sha256"
tar xzf "$NAME.tar.gz"
install -m 0755 "$NAME/mcp-server-git-rs" /usr/local/bin/
```

[releases]: https://github.com/karan-vk/mcp-server-git-rs/releases/latest

**macOS Gatekeeper:** the binaries are not Apple-notarized (a paid Developer ID
account is out of scope for this project). The first run will be blocked. Clear
the quarantine attribute once after install:

```bash
xattr -d com.apple.quarantine /usr/local/bin/mcp-server-git-rs
```

Or right-click the binary in Finder → *Open* → *Open anyway*.

### `cargo install`

```bash
cargo install mcp-server-git-rs
```

### Build from source

```bash
git clone https://github.com/karan-vk/mcp-server-git-rs
cd mcp-server-git-rs
cargo build --release
# binary: target/release/mcp-server-git-rs
```

## Quickstart

```
mcp-server-git-rs [OPTIONS]

  -r, --repository <PATH>   restrict operations to this repo (all tool calls
                            must resolve inside it after canonicalisation)
  -v, --verbose             repeatable: -v info, -vv debug (default: warn)
```

Logs go to stderr; stdout is the MCP channel.

## Use with your agent

`mcp-server-git-rs` speaks MCP over stdio, so anything that can spawn an
MCP-speaking child process can use it. Pick your agent below for a
copy-pasteable config. All snippets assume the binary is on `$PATH` — substitute
an absolute path (e.g. `/usr/local/bin/mcp-server-git-rs`) otherwise, and add
`"args": ["-r", "/path/to/your/repo"]` if you want to scope the server to a
single repository.

| Agent | Config | Has CLI helper |
|---|---|---|
| [Claude Code](#claude-code) | `.mcp.json` / user settings | `claude mcp add` |
| [Claude Desktop](#claude-desktop) | `claude_desktop_config.json` | — |
| [Cursor](#cursor) | `.cursor/mcp.json` | — |
| [Windsurf](#windsurf) | `~/.codeium/windsurf/mcp_config.json` | — |
| [Gemini CLI](#gemini-cli) | `~/.gemini/settings.json` | — |
| [Codex CLI](#codex-cli-openai) | `~/.codex/config.toml` | `codex mcp add` |
| [OpenCode](#opencode) | `opencode.json` | — |
| [Continue.dev](#continuedev) | `~/.continue/config.yaml` | — |
| [Zed](#zed) | `~/.config/zed/settings.json` | — |
| [Cline](#cline-vs-code-extension) | `cline_mcp_settings.json` | — |
| [Roo Code](#roo-code) | `.roo/mcp.json` | — |
| [VS Code native](#vs-code-native-copilot-agent-mode) | `.vscode/mcp.json` | — |

### Claude Code

One-liner via the CLI:

```bash
claude mcp add --transport stdio git-rs -- mcp-server-git-rs
```

`--scope user` makes it visible across projects; `--scope project` commits it to
`.mcp.json` at the repo root. Or write `.mcp.json` directly:

```json
{
  "mcpServers": {
    "git-rs": { "command": "mcp-server-git-rs" }
  }
}
```

Docs: <https://code.claude.com/docs/en/mcp>

### Claude Desktop

Edit `claude_desktop_config.json`:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "git-rs": { "command": "mcp-server-git-rs" }
  }
}
```

Fully quit and relaunch Claude Desktop after editing.

Docs: <https://modelcontextprotocol.io/docs/develop/connect-local-servers>

### Cursor

Project-scoped at `.cursor/mcp.json` (or global at `~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "git-rs": {
      "command": "mcp-server-git-rs",
      "args": []
    }
  }
}
```

Docs: <https://cursor.com/docs/context/mcp>

### Windsurf

`~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "git-rs": {
      "command": "mcp-server-git-rs",
      "args": []
    }
  }
}
```

Docs: <https://docs.windsurf.com/windsurf/cascade/mcp>

### Gemini CLI

`~/.gemini/settings.json` (or `.gemini/settings.json` per-project):

```json
{
  "mcpServers": {
    "git-rs": {
      "command": "mcp-server-git-rs",
      "args": [],
      "timeout": 600000,
      "trust": false
    }
  }
}
```

`trust: true` skips tool-call confirmation for this server; leave `false` unless
you understand the implications.

Docs: <https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md>

### Codex CLI (OpenAI)

One-liner via the CLI:

```bash
codex mcp add git-rs -- mcp-server-git-rs
```

Or edit `~/.codex/config.toml`:

```toml
[mcp_servers.git-rs]
command = "mcp-server-git-rs"
args = []
```

This config is shared between the Codex CLI and the Codex IDE extension.

Docs: <https://developers.openai.com/codex/mcp>

### OpenCode

`opencode.json` at project root (or `~/.config/opencode/opencode.json` globally).
Note the differences from the common pattern: top-level key is `mcp` (not
`mcpServers`), `command` is a single array combining the binary and its args,
and each server needs an explicit `type`.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "git-rs": {
      "type": "local",
      "command": ["mcp-server-git-rs"],
      "enabled": true
    }
  }
}
```

Docs: <https://opencode.ai/docs/mcp-servers/>

### Continue.dev

`~/.continue/config.yaml` (the older `config.json` is deprecated):

```yaml
mcpServers:
  - name: git-rs
    type: stdio
    command: mcp-server-git-rs
    args: []
```

Docs: <https://docs.continue.dev/customize/deep-dives/mcp>

### Zed

`~/.config/zed/settings.json`. Zed's key is `context_servers`, not `mcpServers`:

```json
{
  "context_servers": {
    "git-rs": {
      "source": "custom",
      "command": "mcp-server-git-rs",
      "args": [],
      "env": {}
    }
  }
}
```

Zed auto-restarts the context server on save — no editor restart needed.

Docs: <https://zed.dev/docs/ai/mcp>

### Cline (VS Code extension)

Open the Cline panel → *MCP Servers* → *Configure*, and paste into the file Cline
opens:

```json
{
  "mcpServers": {
    "git-rs": {
      "command": "mcp-server-git-rs",
      "args": [],
      "disabled": false,
      "alwaysAllow": []
    }
  }
}
```

Docs: <https://docs.cline.bot/mcp/configuring-mcp-servers>

### Roo Code

`.roo/mcp.json` at project root (or Roo's global MCP settings):

```json
{
  "mcpServers": {
    "git-rs": {
      "type": "stdio",
      "command": "mcp-server-git-rs",
      "args": [],
      "disabled": false,
      "alwaysAllow": []
    }
  }
}
```

Docs: <https://docs.roocode.com/features/mcp/server-transports>

### VS Code native (Copilot Agent mode)

`.vscode/mcp.json` in a workspace (or user-level via *MCP: Open User
Configuration*). VS Code's key is `servers`, not `mcpServers`:

```json
{
  "servers": {
    "git-rs": {
      "type": "stdio",
      "command": "mcp-server-git-rs",
      "args": []
    }
  }
}
```

Docs: <https://code.visualstudio.com/docs/copilot/customization/mcp-servers>

### A note on Pieces (pi.dev)

Pieces is *itself* an MCP server (it exposes Pieces Long-Term Memory to hosts
like Cursor and Claude Desktop) — it is not an MCP host, so there is no place
inside Pieces to register `mcp-server-git-rs`. Install the Pieces MCP server
alongside `mcp-server-git-rs` in any of the hosts above.

Docs: <https://docs.pieces.app/products/mcp/get-started>

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

## Contributing

Pull requests welcome. Before submitting, please make sure the standard checks
pass locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked --all-targets
```

Bug reports and feature requests are tracked in [GitHub Issues][issues].

[issues]: https://github.com/karan-vk/mcp-server-git-rs/issues

## License

[MIT](LICENSE).
