# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/karan-vk/mcp-server-git-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/karan-vk/mcp-server-git-rs/releases/tag/v0.1.0
