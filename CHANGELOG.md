# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Bumped `rmcp` from 1.5 to 3.10.0. `tools/list` now builds its result via
  `ListToolsResult::with_all_items`; wire output is unchanged (same 52 tools,
  no pagination cursor).
- Bumped `git2` from 0.20 to 0.21 (vendored libgit2 1.9.7). The `ssh` and
  `https` features, no longer on by default in git2, are enabled explicitly
  so push, fetch and ls-remote keep both transports. Non-UTF-8 names, paths
  and messages are handled exactly as before.

### Fixed
- The server now exits with status 0 when input closes before the client
  sends anything that starts a session (rmcp's `ConnectionClosed` during
  the initialize handshake). A host shutting down an unused server is not a
  failure; any other startup error still exits non-zero.
- SSH remote operations (`git_push`, `git_fetch`, `git_ls_remote`,
  `git_tag_push`) no longer hang when no ssh-agent is running. libgit2
  re-invokes the credentials callback after every rejection, and
  `ssh_key_from_agent` succeeds even without an agent, so the old callback
  looped forever. Each auth source is now tried at most once per operation;
  a rejected HTTPS credential moves on to the next source in the chain
  instead of being retried.
- `git_fetch`, `git_ls_remote` and `git_remote_prune` errors now include the
  underlying cause (e.g. `connect to origin failed: SSH auth failed: ...`),
  matching `git_push`.
- `initialize` now reports `serverInfo` as `mcp-server-git-rs` with this
  crate's version, instead of rmcp's own crate name and version.
- `git_add` now follows `git add -- <pathspec>` semantics, matching the
  Python reference: directories recurse, globs expand, and tracked paths
  missing from the working tree stage their removal, so a directory rename
  stages in one call (`["olddir", "newdir"]`). `./path`, `dir/` and absolute
  paths inside the repository are accepted. Previously only existing regular
  files (or `["."]`) could be staged.
- `git_add` parity with the Python reference: an empty `files` list is
  rejected (schema `minItems: 1`), paths resolving outside the repository
  report `Path '...' is outside the repository '...'`, a call that changes
  nothing returns `No changes were staged...`, and unmatched or ignored paths
  error with git's wording. All paths are validated first, so one bad entry
  stages nothing.

## [0.2.1] – 2026-04-25

### Fixed
- `git_push` and `git_tag_push` now fall back to `git credential fill`
  when libgit2's built-in credential-helper path returns nothing. Picks
  up macOS osxkeychain, Windows credential manager, and custom helpers
  like `!gh auth git-credential` that libgit2's own implementation
  doesn't understand. Auth order is now: ssh-agent → libgit2 credential
  helper → `git credential fill` → `$MCP_GIT_TOKEN`.

## [0.2.0] – 2026-04-25

### Added
- `--features <LIST>` CLI flag for enabling additional tool groups at runtime
  (comma-separated; `all` enables everything). Default behavior is unchanged
  — the core 13 tools are exposed when the flag is absent.
- **inspection** group (5 tools): `git_blame`, `git_blame_line`, `git_ls_tree`,
  `git_cat_file`, `git_show_ref`.
- **tags** group (5 tools): `git_tag_list`, `git_tag_create`, `git_tag_delete`,
  `git_tag_push`, `git_describe`.
- **stash** group (6 tools): `git_stash_list`, `git_stash_save`,
  `git_stash_pop`, `git_stash_apply`, `git_stash_drop`, `git_stash_show`.
- **remotes** group (7 tools): `git_remote_list`, `git_remote_add`,
  `git_remote_remove`, `git_remote_set_url`, `git_fetch`, `git_remote_prune`,
  `git_ls_remote`. Auth shares the ssh-agent + credential-helper +
  `$MCP_GIT_TOKEN` chain that already powers `git_push`.
- **history** group (5 tools, mostly destructive): `git_revert`,
  `git_cherry_pick`, `git_reset_hard`, `git_clean`, `git_rev_parse`.
- **branches-extended** group (4 tools): `git_branch_rename`,
  `git_branch_delete`, `git_set_upstream`, `git_merge_base`.
- **worktrees** group (3 tools): `git_worktree_list`, `git_worktree_add`,
  `git_worktree_remove`.
- **notes** group (4 tools): `git_notes_list`, `git_notes_add`,
  `git_notes_remove`, `git_grep`.
- `bench/` — reproducible benchmark harness (Python JSON-RPC driver) with a
  captured baseline in `bench/results/`.

### Changed
- `--repository` (`-r`) is now repeatable. Pass `-r` multiple times to
  scope a single server to several repos; worktrees of any allowed repo
  are auto-allowed even at sibling paths. Without the flag the server is
  unscoped, same as before. Single-`-r` behavior is unchanged.
- README benchmark section rewritten to document methodology, hardware, and
  fixture SHAs alongside the numbers. The previous "~60×" handwave is
  replaced with per-tool p50/p95 against the Python reference server, all
  reproducible via `bench/run.py`.
- README tool table split into a "core" section and 8 optional groups.

## [0.1.0] – 2026-04-25

### Added
- Initial release. Rust port of the Python `mcp-server-git` reference server
  with matching tool names, schemas, and stdio transport.
- Twelve read/write tools: `git_status`, `git_diff_unstaged`, `git_diff_staged`,
  `git_diff`, `git_commit`, `git_add`, `git_reset`, `git_log`,
  `git_create_branch`, `git_checkout`, `git_show`, `git_branch`.
- New `git_push` tool with ssh-agent + credential-helper authentication,
  opt-in force push, and `set_upstream` support.
- `--repository` flag for canonicalised repo scoping; flag-injection defense
  on all user-provided refs and paths.
- ISO-8601 and relative-span timestamp parsing in `git_log` via `jiff`.

[Unreleased]: https://github.com/karan-vk/mcp-server-git-rs/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/karan-vk/mcp-server-git-rs/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/karan-vk/mcp-server-git-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/karan-vk/mcp-server-git-rs/releases/tag/v0.1.0
