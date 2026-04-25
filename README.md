# mcp-server-git-rs

[![Crates.io](https://img.shields.io/crates/v/mcp-server-git-rs.svg)](https://crates.io/crates/mcp-server-git-rs)
[![CI](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/ci.yml)
[![Release](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/release.yml/badge.svg)](https://github.com/karan-vk/mcp-server-git-rs/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast Rust MCP server that exposes git repository operations over stdio. Drop-in
replacement for the Python [`mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
reference server: same tool names, same input schemas, same stdio transport.

Single static binary. On an Apple M3, ~5 ms cold start vs ~375 ms for the
Python server, and warm `tools/call` round-trips of 1–7 ms across the
read-only tools we measure. Numbers, methodology, and a reproducible harness
are in [`bench/`](./bench).

## Highlights

- **52 tools total** — the full Python reference set plus `git_push`, plus 39
  more across 8 opt-in groups (inspection, tags, stash, remotes, history,
  branches-extended, worktrees, notes). Default invocation still exposes only
  the **core 13** so smaller agents don't see a 50-tool list.
- **Staged via `--features`** — one CLI flag, comma-separated values, `all`
  shorthand. Single binary, gated at runtime.
- **Reproducible benchmarks** — `bench/run.py` measures cold start + warm-call
  p50/p95 against `uvx mcp-server-git` on whatever fixture you point it at.
  The "60×" claim is now a number you can verify, not a marketing line.
- **Security-hardened** — canonicalised repo scoping (`--repository`),
  flag-injection rejection, `revparse` validation. Matches the Python server's
  invariants one-to-one and applies them to every new tool.
- **Works with 12+ MCP-speaking agents** — see [Use with your agent](#use-with-your-agent).

## Tools

### Core (always on)

The default invocation exposes these 13 tools — the full Python reference set
plus `git_push`. They match the Python server's names, schemas, and error
shapes one-to-one.

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
| `git_push` *(new in v0.1.0)* | Push to a remote. SSH agent for `git@`, credential helper for `https://` | destructive |

### Optional groups (`--features <LIST>`)

Pass `--features` with a comma-separated list to enable additional groups, or
`--features all` to enable everything. Without the flag, the core 13 above are
the only tools exposed.

| Group | Flag | Tools |
|---|---|---|
| Inspection | `inspection` | `git_blame`, `git_blame_line`, `git_ls_tree`, `git_cat_file`, `git_show_ref` |
| Tags | `tags` | `git_tag_list`, `git_tag_create`, `git_tag_delete`, `git_tag_push`, `git_describe` |
| Stash | `stash` | `git_stash_list`, `git_stash_save`, `git_stash_pop`, `git_stash_apply`, `git_stash_drop`, `git_stash_show` |
| Remotes / network | `remotes` | `git_remote_list`, `git_remote_add`, `git_remote_remove`, `git_remote_set_url`, `git_fetch`, `git_remote_prune`, `git_ls_remote` |
| History (mostly destructive) | `history` | `git_revert`, `git_cherry_pick`, `git_reset_hard`, `git_clean`, `git_rev_parse` |
| Branches (extended) | `branches-extended` | `git_branch_rename`, `git_branch_delete`, `git_set_upstream`, `git_merge_base` |
| Worktrees | `worktrees` | `git_worktree_list`, `git_worktree_add`, `git_worktree_remove` |
| Notes / grep | `notes` | `git_notes_list`, `git_notes_add`, `git_notes_remove`, `git_grep` |

Examples:

```bash
mcp-server-git-rs                                 # core 13 tools only
mcp-server-git-rs --features inspection,stash    # core + 11 tools
mcp-server-git-rs --features all                 # all 52 tools
```

A disabled group's tools are hidden from `tools/list`; calling one anyway
returns an `invalid_request` error naming the missing feature.

## Install

### Prebuilt binary (fastest)

Grab the tarball for your platform from the [latest release][releases]. Each
tarball extracts to a folder containing the binary, `LICENSE`, and `README.md`,
and ships with a `.sha256` sibling for verification.

```bash
# Pick the triple that matches your machine
TAG=v0.2.0
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

  -r, --repository <PATH>   restrict operations to this repo. Repeatable —
                            pass `-r` multiple times to allow several repos.
                            Worktrees of allowed repos are auto-allowed
                            even at sibling paths.
      --features <LIST>     enable additional tool groups (comma-separated;
                            inspection, tags, stash, remotes, history,
                            branches-extended, worktrees, notes, all)
  -v, --verbose             repeatable: -v info, -vv debug (default: warn)
```

Logs go to stderr; stdout is the MCP channel.

### Multiple repos, branches, and worktrees

The server is stateless per call — every tool takes its own `repo_path`,
and branch / revision are also per-call arguments. One server instance can
freely operate on any number of repos and branches in the same session:

| Invocation | What's allowed |
|---|---|
| `mcp-server-git-rs` | unscoped — any `repo_path`, any branch, any worktree |
| `mcp-server-git-rs -r /a` | repo A only, plus any branch in A and any worktree of A |
| `mcp-server-git-rs -r /a -r /b -r /c` | A, B, C (and their worktrees), per call |

Worktrees of an allowed repo are accepted automatically even when their
working directory lives outside the allowed root (which is the normal
layout, since `git worktree add` typically creates a sibling directory).
The check resolves the candidate's `commondir` and accepts it if that
maps under any allowed root.

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

Numbers, methodology, and the harness live in [`bench/`](./bench). What we
measure:

1. **Cold start** — `Popen` → response to `initialize`. The latency a host
   like Claude Desktop pays on every fresh launch.
2. **Warm-call latency** — `tools/call` round-trip over a long-running stdio
   session that already finished `initialize`. The cost an agent loop pays
   per tool invocation.

We compare against `uvx mcp-server-git` on the read-only tools that overlap
between the two servers. The harness runs locally:

```bash
cargo build --release
cd bench && python3 run.py --out results/$(date +%Y-%m-%d).md
```

### Results — 2026-04-25

Run on an Apple M3 (Darwin 25.3.0 arm64), `rustc 1.94.0`, Python 3.13.12,
`uvx 0.9.6`. Fixture is this repo at the commit listed in
[`bench/results/2026-04-25.md`](./bench/results/2026-04-25.md).

**Cold start** (server spawn → `initialize` response, 20 runs):

| Server | p50 (ms) | p95 (ms) | mean (ms) |
|---|---:|---:|---:|
| `mcp-server-git-rs` | 4.78 | 5.20 | 5.46 |
| `mcp-server-git` (python via uvx) | 374.44 | 381.33 | 383.95 |

**Warm-call latency** (one stdio session, 100 iterations after 10 warmup):

| Tool | rust p50 | rust p95 | python p50 | python p95 | speedup p50 |
|---|---:|---:|---:|---:|---:|
| `git_status` | 2.45 | 2.89 | 27.52 | 29.81 | 11.2× |
| `git_log` | 1.18 | 1.39 | 75.97 | 82.84 | 64.6× |
| `git_branch` | 1.17 | 1.28 | 26.47 | 29.20 | 22.6× |
| `git_diff_unstaged` | 6.73 | 7.21 | 33.90 | 34.69 | 5.0× |
| `git_diff_staged` | 1.40 | 1.55 | 25.97 | 27.08 | 18.6× |
| `git_show` | 3.24 | 3.58 | 75.97 | 79.68 | 23.5× |

Caveats: this is one machine, one (small) fixture repo. The harness passes
`branch_type: "local"` to `git_branch` so both servers do equivalent work —
the Python server's schema requires it while Rust defaults to local; without
the explicit argument Python returns a fast schema-validation error and the
comparison is meaningless. Re-run the harness on your own setup before
quoting these numbers; please file an issue with results from other
hardware.

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
