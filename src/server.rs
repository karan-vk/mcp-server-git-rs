use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::guard::validate_repo_path;
use crate::log::git_log;
use crate::push::{git_push, PushArgs};
use crate::tools;

fn default_context_lines() -> u32 {
    tools::DEFAULT_CONTEXT_LINES
}
fn default_max_count() -> usize {
    10
}
fn default_remote() -> String {
    "origin".into()
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RepoOnly {
    /// Absolute path to the Git repository.
    pub repo_path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DiffNoTarget {
    pub repo_path: String,
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DiffWithTarget {
    pub repo_path: String,
    /// Target branch, tag, or commit to diff against.
    pub target: String,
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CommitParams {
    pub repo_path: String,
    pub message: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddParams {
    pub repo_path: String,
    /// Paths to stage. Pass `["."]` to stage everything.
    pub files: Vec<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LogParams {
    pub repo_path: String,
    #[serde(default = "default_max_count")]
    pub max_count: usize,
    /// Optional start timestamp. Accepts ISO-8601 (`2024-01-15`, `2024-01-15T14:30:25`)
    /// or relative (`2 weeks ago`, `3 days`).
    #[serde(default)]
    pub start_timestamp: Option<String>,
    /// Optional end timestamp. Same formats as start_timestamp.
    #[serde(default)]
    pub end_timestamp: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateBranchParams {
    pub repo_path: String,
    pub branch_name: String,
    #[serde(default)]
    pub base_branch: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CheckoutParams {
    pub repo_path: String,
    pub branch_name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ShowParams {
    pub repo_path: String,
    pub revision: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BranchParams {
    pub repo_path: String,
    /// `"local"`, `"remote"`, or `"all"`.
    pub branch_type: String,
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub not_contains: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PushParams {
    pub repo_path: String,
    #[serde(default = "default_remote")]
    pub remote: String,
    /// Branch to push. Defaults to the current HEAD's branch.
    #[serde(default)]
    pub branch: Option<String>,
    /// Force-push. Overwrites remote history. Use with caution.
    #[serde(default)]
    pub force: bool,
    /// Write `branch.<name>.remote` and `branch.<name>.merge` in local config.
    #[serde(default)]
    pub set_upstream: bool,
}

#[derive(Clone)]
pub struct GitServer {
    allowed_root: Option<Arc<PathBuf>>,
}

impl GitServer {
    pub fn new(allowed_root: Option<PathBuf>) -> Self {
        Self {
            allowed_root: allowed_root.map(Arc::new),
        }
    }

    fn open(&self, repo_path: &str) -> Result<git2::Repository, ErrorData> {
        let root: Option<&Path> = self.allowed_root.as_deref().map(|p| p.as_path());
        let canonical = validate_repo_path(repo_path, root).map_err(to_error)?;
        tools::open_repo(&canonical).map_err(to_error)
    }
}

fn to_error(err: anyhow::Error) -> ErrorData {
    ErrorData::invalid_params(err.to_string(), None)
}

fn into_text(result: Result<String>) -> Result<String, ErrorData> {
    result.map_err(to_error)
}

#[tool_router(server_handler)]
impl GitServer {
    #[tool(
        name = "git_status",
        description = "Shows the working tree status",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_status(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_status(&repo).map(|s| format!("Repository status:\n{s}")))
    }

    #[tool(
        name = "git_diff_unstaged",
        description = "Shows changes in the working directory that are not yet staged",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_diff_unstaged(
        &self,
        Parameters(p): Parameters<DiffNoTarget>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(
            tools::git_diff_unstaged(&repo, p.context_lines)
                .map(|d| format!("Unstaged changes:\n{d}")),
        )
    }

    #[tool(
        name = "git_diff_staged",
        description = "Shows changes that are staged for commit",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_diff_staged(
        &self,
        Parameters(p): Parameters<DiffNoTarget>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(
            tools::git_diff_staged(&repo, p.context_lines).map(|d| format!("Staged changes:\n{d}")),
        )
    }

    #[tool(
        name = "git_diff",
        description = "Shows differences between the working tree and a branch or commit",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_diff(
        &self,
        Parameters(p): Parameters<DiffWithTarget>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        let target = p.target.clone();
        into_text(
            tools::git_diff(&repo, &p.target, p.context_lines)
                .map(|d| format!("Diff with {target}:\n{d}")),
        )
    }

    #[tool(
        name = "git_commit",
        description = "Records changes to the repository",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_commit(
        &self,
        Parameters(p): Parameters<CommitParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_commit(&repo, &p.message))
    }

    #[tool(
        name = "git_add",
        description = "Adds file contents to the staging area",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_add(
        &self,
        Parameters(p): Parameters<AddParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_add(&repo, &p.files))
    }

    #[tool(
        name = "git_reset",
        description = "Unstages all staged changes",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_reset(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_reset(&repo))
    }

    #[tool(
        name = "git_log",
        description = "Shows the commit logs with optional date filtering",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_log(
        &self,
        Parameters(p): Parameters<LogParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        let body = git_log(
            &repo,
            p.max_count,
            p.start_timestamp.as_deref(),
            p.end_timestamp.as_deref(),
        );
        into_text(body.map(|b| format!("Commit history:\n{b}")))
    }

    #[tool(
        name = "git_create_branch",
        description = "Creates a new branch from an optional base branch",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_create_branch(
        &self,
        Parameters(p): Parameters<CreateBranchParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_create_branch(
            &repo,
            &p.branch_name,
            p.base_branch.as_deref(),
        ))
    }

    #[tool(
        name = "git_checkout",
        description = "Switches branches",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_checkout(
        &self,
        Parameters(p): Parameters<CheckoutParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_checkout(&repo, &p.branch_name))
    }

    #[tool(
        name = "git_show",
        description = "Shows the contents of a commit",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_show(
        &self,
        Parameters(p): Parameters<ShowParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_show(&repo, &p.revision))
    }

    #[tool(
        name = "git_branch",
        description = "List Git branches",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_branch(
        &self,
        Parameters(p): Parameters<BranchParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(tools::git_branch(
            &repo,
            &p.branch_type,
            p.contains.as_deref(),
            p.not_contains.as_deref(),
        ))
    }

    #[tool(
        name = "git_push",
        description = "Push a local branch to a remote. Uses SSH agent for git@ remotes and the system credential helper (or MCP_GIT_TOKEN env var) for https://.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn tool_git_push(
        &self,
        Parameters(p): Parameters<PushParams>,
    ) -> Result<String, ErrorData> {
        let repo = self.open(&p.repo_path)?;
        into_text(git_push(
            &repo,
            PushArgs {
                remote: &p.remote,
                branch: p.branch.as_deref(),
                force: p.force,
                set_upstream: p.set_upstream,
            },
        ))
    }
}
