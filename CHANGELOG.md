# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
