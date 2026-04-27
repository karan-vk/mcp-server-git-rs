---
name: git-rs
description: Use this skill whenever git operations are needed and the `mcp__git-rs__*` tools are connected. Routes git work through the mcp-server-git-rs MCP instead of shelling out to `git` — typically 5–60× faster, returns structured output, and respects repo-scoping. Covers commit / branch / log / diff / status / push workflows, the 8 feature-gated groups (inspection, tags, stash, remotes, history, branches-extended, worktrees, notes), the ssh-agent / credential-helper / `$MCP_GIT_TOKEN` auth chain, and the timestamp / flag-injection / repo-path constraints the server enforces.
---

# git-rs MCP skill

When `mcp__git-rs__*` tools are visible in your tool list, **prefer them over running `git` via Bash** for any operation the server exposes. They are faster, return structured text the model can parse cleanly, and respect the server's repo-scoping rules. Reserve `git` over Bash for the operations the MCP server doesn't cover (see "When NOT to use this MCP" below).

## Tool discovery

Tool availability depends on how the user launched the server:

- **Default** (no `--features`): only the **core 13** are exposed — `git_status`, `git_diff_unstaged`, `git_diff_staged`, `git_diff`, `git_commit`, `git_add`, `git_reset`, `git_log`, `git_create_branch`, `git_checkout`, `git_show`, `git_branch`, `git_push`.
- **With `--features <group>` or `--features all`**: additional tools appear. The 8 groups are `inspection`, `tags`, `stash`, `remotes`, `history`, `branches-extended`, `worktrees`, `notes`.

If a tool call returns `invalid_request` with `"tool gated by --features <group>"`, that group is not enabled. **Don't ask the user to restart their agent** — fall back to the core tools or shell out to `git` for that one operation.

## Common workflows

### Commit a change
```
mcp__git-rs__git_status        { repo_path }                                       # check state first
mcp__git-rs__git_diff_unstaged { repo_path }                                       # review changes
mcp__git-rs__git_add           { repo_path, files: ["src/foo.rs", "src/bar.rs"] } # stage specific files
mcp__git-rs__git_diff_staged   { repo_path }                                       # confirm what's staged
mcp__git-rs__git_commit        { repo_path, message: "…" }
```

Avoid `files: ["."]` unless the user explicitly asked to stage everything; selective staging is the default.

### Inspect history
```
mcp__git-rs__git_log   { repo_path, max_count: 20 }
mcp__git-rs__git_log   { repo_path, start_timestamp: "2 weeks ago" }
mcp__git-rs__git_show  { repo_path, revision: "HEAD~1" }
mcp__git-rs__git_diff  { repo_path, target: "main" }
```

### Branch / checkout
```
mcp__git-rs__git_branch        { repo_path, branch_type: "all" }
mcp__git-rs__git_create_branch { repo_path, branch_name: "feature/x", base_branch: "main" }
mcp__git-rs__git_checkout      { repo_path, branch_name: "feature/x" }
```

### Push
```
mcp__git-rs__git_push { repo_path, remote: "origin", branch: "feature/x", set_upstream: true }
```

Auth chain for push / fetch / tag-push: **ssh-agent (for `git@`) → libgit2 credential helper → `git credential fill` → `$MCP_GIT_TOKEN`**. If push fails with an auth error, the user's credentials aren't reachable — surface that clearly and tell them to load their key into ssh-agent or set `MCP_GIT_TOKEN`. Don't retry blindly.

## Hard constraints — these are wall-clock failures, not warnings

- **`repo_path` must be an absolute path.** The server canonicalises it. `.` and other relative paths will fail. If the user gave you a relative path, resolve it to absolute before calling.
- **No flag-like values.** Refs, branch names, timestamps, and patterns starting with `-` are rejected before reaching libgit2 (flag-injection defense). If a real ref starts with a dash, that's a bug in the caller — don't try to escape it.
- **`git_log` timestamps**: ISO-8601 (`2024-01-15`, `2024-01-15T14:30:25Z`) and `<N> <unit> [ago]` (`2 weeks ago`, `3 days`, `1 month ago`). Free-form approxidate (`yesterday`, `last tuesday`, `noon`) is **not** supported — convert to ISO before calling.
- **`git_branch` `branch_type`** must be one of `"local"`, `"remote"`, `"all"` (default `"all"`).
- **`git_clean`** requires `force: true` explicitly; otherwise it errors by design.

## Destructive tools — confirm before calling

These mutate or discard work:

- `git_reset` — unstages everything. (Core; usually safe but the user may want a partial unstage instead.)
- `git_push` with `force: true` — overwrites remote history.
- `git_reset_hard` — discards local work. (`history` group.)
- `git_clean { force: true }` — deletes untracked files. (`history` group.)
- `git_revert`, `git_cherry_pick` — write new commits. (`history` group.)
- `git_branch_delete`, `git_tag_delete`, `git_stash_drop`, `git_stash_pop`, `git_remote_remove`, `git_remote_prune`, `git_worktree_remove`, `git_notes_remove` — irreversible deletions of refs / stash entries / worktrees.

Confirm intent with the user before invoking any of these on real work — auto mode is not a license to discard their changes.

## When NOT to use this MCP

Shell out via Bash for these — the server doesn't expose them:

- `git rebase` (any flavour), `git bisect`, `git submodule`, `git sparse-checkout`, hooks management.
- Reading or editing `.gitignore`, `.git/config`, or anything inside `.git/` directly — use `Read` / `Edit`.
- Rendered output the user explicitly wants to see (`git log --oneline --graph`, `git diff --stat`) — the MCP returns structured text, which is great for parsing but not for paste-back.

## Output conventions

Tools return human-readable text mirroring the Python `mcp-server-git` reference server, often prefixed with a header line (`Repository status:\n…`, `Diff with main:\n…`, `Commit history:\n…`). Don't strip those headers when echoing results back to the user — they are part of the contract.
