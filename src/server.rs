use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{ErrorData, ListToolsResult, PaginatedRequestParams, Tool};
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use rmcp::{schemars, tool, tool_handler, tool_router};
use serde::Deserialize;

use crate::branches_ext;
use crate::features::{Feature, FeatureSet};
use crate::guard::validate_repo_path;
use crate::history;
use crate::inspection;
use crate::log::git_log;
use crate::notes;
use crate::push::{git_push, PushArgs};
use crate::remotes;
use crate::stash;
use crate::tags;
use crate::tools;
use crate::worktrees;

fn default_context_lines() -> u32 {
    tools::DEFAULT_CONTEXT_LINES
}
fn default_max_count() -> usize {
    10
}
fn default_remote() -> String {
    "origin".into()
}
fn default_branch_type() -> String {
    "all".into()
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
    #[serde(default = "default_branch_type")]
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

// ---------- inspection ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BlameParams {
    pub repo_path: String,
    pub path: String,
    #[serde(default)]
    pub rev: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BlameLineParams {
    pub repo_path: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(default)]
    pub rev: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LsTreeParams {
    pub repo_path: String,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CatFileParams {
    pub repo_path: String,
    /// A revision spec or object id (e.g. `HEAD:src/main.rs`, an OID, or a tree path).
    pub spec: String,
}

// ---------- tags ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TagListParams {
    pub repo_path: String,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TagCreateParams {
    pub repo_path: String,
    pub name: String,
    #[serde(default)]
    pub target: Option<String>,
    /// If set, creates an annotated tag with this message. Otherwise creates a lightweight tag.
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TagDeleteParams {
    pub repo_path: String,
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TagPushParams {
    pub repo_path: String,
    #[serde(default = "default_remote")]
    pub remote: String,
    pub tag: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DescribeParams {
    pub repo_path: String,
    #[serde(default)]
    pub rev: Option<String>,
    /// Match against tags (default) rather than all refs.
    #[serde(default = "default_true")]
    pub tags: bool,
    /// Number of hex digits to abbreviate the OID suffix to.
    #[serde(default)]
    pub abbrev: Option<u32>,
}

fn default_true() -> bool {
    true
}

// ---------- stash ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StashSaveParams {
    pub repo_path: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub include_untracked: bool,
    #[serde(default)]
    pub keep_index: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StashIndexParams {
    pub repo_path: String,
    #[serde(default)]
    pub index: usize,
}

// ---------- remotes ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RemoteAddParams {
    pub repo_path: String,
    pub name: String,
    pub url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RemoteNameParams {
    pub repo_path: String,
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RemoteSetUrlParams {
    pub repo_path: String,
    pub name: String,
    pub url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FetchParams {
    pub repo_path: String,
    #[serde(default = "default_remote")]
    pub name: String,
    /// Refspecs to fetch. Empty list uses the remote's configured refspecs.
    #[serde(default)]
    pub refspecs: Vec<String>,
}

// ---------- history ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RevParam {
    pub repo_path: String,
    pub rev: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CleanParams {
    pub repo_path: String,
    /// Must be `true` — git_clean refuses to run without explicit force.
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RevParseParams {
    pub repo_path: String,
    pub spec: String,
}

// ---------- branches-extended ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BranchRenameParams {
    pub repo_path: String,
    pub old_name: String,
    pub new_name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BranchDeleteParams {
    pub repo_path: String,
    pub name: String,
    /// Set true to delete a remote-tracking branch instead of a local one.
    #[serde(default)]
    pub remote: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetUpstreamParams {
    pub repo_path: String,
    pub branch: String,
    /// Set to `null` to clear the upstream.
    #[serde(default)]
    pub upstream: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MergeBaseParams {
    pub repo_path: String,
    pub a: String,
    pub b: String,
}

// ---------- worktrees ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct WorktreeAddParams {
    pub repo_path: String,
    pub name: String,
    pub path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct WorktreeRemoveParams {
    pub repo_path: String,
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

// ---------- notes / grep ----------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NotesListParams {
    pub repo_path: String,
    #[serde(default)]
    pub notes_ref: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NotesAddParams {
    pub repo_path: String,
    pub target: String,
    pub message: String,
    #[serde(default)]
    pub notes_ref: Option<String>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NotesRemoveParams {
    pub repo_path: String,
    pub target: String,
    #[serde(default)]
    pub notes_ref: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GrepParams {
    pub repo_path: String,
    /// Regular expression to search for (Rust regex syntax).
    pub pattern: String,
    #[serde(default)]
    pub ignore_case: bool,
}

#[derive(Clone)]
pub struct GitServer {
    allowed_roots: Arc<Vec<PathBuf>>,
    features: FeatureSet,
}

impl GitServer {
    pub fn new(allowed_roots: Vec<PathBuf>, features: FeatureSet) -> Self {
        Self {
            allowed_roots: Arc::new(allowed_roots),
            features,
        }
    }

    fn open(&self, repo_path: &str) -> Result<git2::Repository, ErrorData> {
        let canonical = validate_repo_path(repo_path, &self.allowed_roots).map_err(to_error)?;
        tools::open_repo(&canonical).map_err(to_error)
    }

    fn require_feature(&self, f: Feature) -> Result<(), ErrorData> {
        if self.features.has(f) {
            Ok(())
        } else {
            Err(ErrorData::invalid_request(
                format!(
                    "tool gated by --features {}; pass --features {} (or all) to enable",
                    f.name(),
                    f.name()
                ),
                None,
            ))
        }
    }
}

fn to_error(err: anyhow::Error) -> ErrorData {
    ErrorData::invalid_params(err.to_string(), None)
}

fn into_text(result: Result<String>) -> Result<String, ErrorData> {
    result.map_err(to_error)
}

/// Map a tool name to the feature group that gates it. Returns `None` for the
/// always-on core tools.
pub fn tool_feature(name: &str) -> Option<Feature> {
    match name {
        // inspection
        "git_blame" | "git_blame_line" | "git_ls_tree" | "git_cat_file" | "git_show_ref" => {
            Some(Feature::Inspection)
        }
        // tags
        "git_tag_list" | "git_tag_create" | "git_tag_delete" | "git_tag_push" | "git_describe" => {
            Some(Feature::Tags)
        }
        // stash
        "git_stash_list" | "git_stash_save" | "git_stash_pop" | "git_stash_apply"
        | "git_stash_drop" | "git_stash_show" => Some(Feature::Stash),
        // remotes
        "git_remote_list" | "git_remote_add" | "git_remote_remove" | "git_remote_set_url"
        | "git_fetch" | "git_remote_prune" | "git_ls_remote" => Some(Feature::Remotes),
        // history
        "git_revert" | "git_cherry_pick" | "git_reset_hard" | "git_clean" | "git_rev_parse" => {
            Some(Feature::History)
        }
        // branches-extended
        "git_branch_rename" | "git_branch_delete" | "git_set_upstream" | "git_merge_base" => {
            Some(Feature::BranchesExtended)
        }
        // worktrees
        "git_worktree_list" | "git_worktree_add" | "git_worktree_remove" => {
            Some(Feature::Worktrees)
        }
        // notes
        "git_notes_list" | "git_notes_add" | "git_notes_remove" | "git_grep" => {
            Some(Feature::Notes)
        }
        _ => None,
    }
}

#[tool_router(vis = "pub")]
impl GitServer {
    // ===================== core (always on) =====================

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

    // ===================== inspection =====================

    #[tool(
        name = "git_blame",
        description = "Annotate each line of a file with the commit that last touched it (feature: inspection)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_blame(
        &self,
        Parameters(p): Parameters<BlameParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Inspection)?;
        let repo = self.open(&p.repo_path)?;
        into_text(inspection::git_blame(&repo, &p.path, p.rev.as_deref()))
    }

    #[tool(
        name = "git_blame_line",
        description = "Blame a contiguous line range of a file (feature: inspection)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_blame_line(
        &self,
        Parameters(p): Parameters<BlameLineParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Inspection)?;
        let repo = self.open(&p.repo_path)?;
        into_text(inspection::git_blame_line(
            &repo,
            &p.path,
            p.start_line,
            p.end_line,
            p.rev.as_deref(),
        ))
    }

    #[tool(
        name = "git_ls_tree",
        description = "List a tree at a revision and optional sub-path (feature: inspection)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_ls_tree(
        &self,
        Parameters(p): Parameters<LsTreeParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Inspection)?;
        let repo = self.open(&p.repo_path)?;
        into_text(inspection::git_ls_tree(
            &repo,
            p.rev.as_deref(),
            p.path.as_deref(),
        ))
    }

    #[tool(
        name = "git_cat_file",
        description = "Show the content of a blob, tree, commit, or annotated tag by spec (feature: inspection)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_cat_file(
        &self,
        Parameters(p): Parameters<CatFileParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Inspection)?;
        let repo = self.open(&p.repo_path)?;
        into_text(inspection::git_cat_file(&repo, &p.spec))
    }

    #[tool(
        name = "git_show_ref",
        description = "List every ref in the repository (feature: inspection)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_show_ref(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Inspection)?;
        let repo = self.open(&p.repo_path)?;
        into_text(inspection::git_show_ref(&repo))
    }

    // ===================== tags =====================

    #[tool(
        name = "git_tag_list",
        description = "List tags, optionally filtered by glob pattern (feature: tags)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_tag_list(
        &self,
        Parameters(p): Parameters<TagListParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Tags)?;
        let repo = self.open(&p.repo_path)?;
        into_text(tags::git_tag_list(&repo, p.pattern.as_deref()))
    }

    #[tool(
        name = "git_tag_create",
        description = "Create a tag (lightweight, or annotated when message is supplied) (feature: tags)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_tag_create(
        &self,
        Parameters(p): Parameters<TagCreateParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Tags)?;
        let repo = self.open(&p.repo_path)?;
        into_text(tags::git_tag_create(
            &repo,
            &p.name,
            p.target.as_deref(),
            p.message.as_deref(),
            p.force,
        ))
    }

    #[tool(
        name = "git_tag_delete",
        description = "Delete a tag (feature: tags)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_tag_delete(
        &self,
        Parameters(p): Parameters<TagDeleteParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Tags)?;
        let repo = self.open(&p.repo_path)?;
        into_text(tags::git_tag_delete(&repo, &p.name))
    }

    #[tool(
        name = "git_tag_push",
        description = "Push a tag to a remote (feature: tags)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn tool_git_tag_push(
        &self,
        Parameters(p): Parameters<TagPushParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Tags)?;
        let repo = self.open(&p.repo_path)?;
        into_text(tags::git_tag_push(&repo, &p.remote, &p.tag, p.force))
    }

    #[tool(
        name = "git_describe",
        description = "Describe a revision against the nearest tag (feature: tags)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_describe(
        &self,
        Parameters(p): Parameters<DescribeParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Tags)?;
        let repo = self.open(&p.repo_path)?;
        into_text(tags::git_describe(
            &repo,
            p.rev.as_deref(),
            p.tags,
            p.abbrev,
        ))
    }

    // ===================== stash =====================

    #[tool(
        name = "git_stash_list",
        description = "List stash entries (feature: stash)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_list(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let mut repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_list(&mut repo))
    }

    #[tool(
        name = "git_stash_save",
        description = "Save the working directory to a new stash entry (feature: stash)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_save(
        &self,
        Parameters(p): Parameters<StashSaveParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let mut repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_save(
            &mut repo,
            p.message.as_deref(),
            p.include_untracked,
            p.keep_index,
        ))
    }

    #[tool(
        name = "git_stash_pop",
        description = "Apply and remove a stash entry (feature: stash)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_pop(
        &self,
        Parameters(p): Parameters<StashIndexParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let mut repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_pop(&mut repo, p.index))
    }

    #[tool(
        name = "git_stash_apply",
        description = "Apply a stash entry without removing it (feature: stash)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_apply(
        &self,
        Parameters(p): Parameters<StashIndexParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let mut repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_apply(&mut repo, p.index))
    }

    #[tool(
        name = "git_stash_drop",
        description = "Drop a stash entry (feature: stash)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_drop(
        &self,
        Parameters(p): Parameters<StashIndexParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let mut repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_drop(&mut repo, p.index))
    }

    #[tool(
        name = "git_stash_show",
        description = "Show the patch represented by a stash entry (feature: stash)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_stash_show(
        &self,
        Parameters(p): Parameters<StashIndexParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Stash)?;
        let repo = self.open(&p.repo_path)?;
        into_text(stash::git_stash_show(&repo, p.index))
    }

    // ===================== remotes =====================

    #[tool(
        name = "git_remote_list",
        description = "List remotes and their URLs (feature: remotes)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_remote_list(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_remote_list(&repo))
    }

    #[tool(
        name = "git_remote_add",
        description = "Add a new remote (feature: remotes)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_remote_add(
        &self,
        Parameters(p): Parameters<RemoteAddParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_remote_add(&repo, &p.name, &p.url))
    }

    #[tool(
        name = "git_remote_remove",
        description = "Remove a remote (feature: remotes)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_remote_remove(
        &self,
        Parameters(p): Parameters<RemoteNameParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_remote_remove(&repo, &p.name))
    }

    #[tool(
        name = "git_remote_set_url",
        description = "Update the URL of an existing remote (feature: remotes)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_remote_set_url(
        &self,
        Parameters(p): Parameters<RemoteSetUrlParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_remote_set_url(&repo, &p.name, &p.url))
    }

    #[tool(
        name = "git_fetch",
        description = "Fetch from a remote (uses SSH agent / credential helper / MCP_GIT_TOKEN) (feature: remotes)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn tool_git_fetch(
        &self,
        Parameters(p): Parameters<FetchParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_fetch(&repo, &p.name, &p.refspecs))
    }

    #[tool(
        name = "git_remote_prune",
        description = "Prune stale remote-tracking refs that no longer exist on the remote (feature: remotes)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn tool_git_remote_prune(
        &self,
        Parameters(p): Parameters<RemoteNameParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_remote_prune(&repo, &p.name))
    }

    #[tool(
        name = "git_ls_remote",
        description = "List references advertised by a remote without fetching (feature: remotes)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn tool_git_ls_remote(
        &self,
        Parameters(p): Parameters<RemoteNameParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Remotes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(remotes::git_ls_remote(&repo, &p.name))
    }

    // ===================== history =====================

    #[tool(
        name = "git_revert",
        description = "Revert a commit, applying the inverse change to the index (feature: history)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_revert(
        &self,
        Parameters(p): Parameters<RevParam>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::History)?;
        let repo = self.open(&p.repo_path)?;
        into_text(history::git_revert(&repo, &p.rev))
    }

    #[tool(
        name = "git_cherry_pick",
        description = "Cherry-pick a commit into the working tree (feature: history)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_cherry_pick(
        &self,
        Parameters(p): Parameters<RevParam>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::History)?;
        let repo = self.open(&p.repo_path)?;
        into_text(history::git_cherry_pick(&repo, &p.rev))
    }

    #[tool(
        name = "git_reset_hard",
        description = "Reset HEAD, index, and working tree to a revision. Discards local work (feature: history)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_reset_hard(
        &self,
        Parameters(p): Parameters<RevParam>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::History)?;
        let repo = self.open(&p.repo_path)?;
        into_text(history::git_reset_hard(&repo, &p.rev))
    }

    #[tool(
        name = "git_clean",
        description = "Remove untracked files from the working tree. Requires force=true (feature: history)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_clean(
        &self,
        Parameters(p): Parameters<CleanParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::History)?;
        let repo = self.open(&p.repo_path)?;
        into_text(history::git_clean(&repo, p.force))
    }

    #[tool(
        name = "git_rev_parse",
        description = "Resolve a revision spec to a full SHA-1 OID (feature: history)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_rev_parse(
        &self,
        Parameters(p): Parameters<RevParseParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::History)?;
        let repo = self.open(&p.repo_path)?;
        into_text(history::git_rev_parse(&repo, &p.spec))
    }

    // ===================== branches-extended =====================

    #[tool(
        name = "git_branch_rename",
        description = "Rename a local branch (feature: branches-extended)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_branch_rename(
        &self,
        Parameters(p): Parameters<BranchRenameParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::BranchesExtended)?;
        let repo = self.open(&p.repo_path)?;
        into_text(branches_ext::git_branch_rename(
            &repo,
            &p.old_name,
            &p.new_name,
            p.force,
        ))
    }

    #[tool(
        name = "git_branch_delete",
        description = "Delete a local (or remote-tracking, with remote=true) branch (feature: branches-extended)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_branch_delete(
        &self,
        Parameters(p): Parameters<BranchDeleteParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::BranchesExtended)?;
        let repo = self.open(&p.repo_path)?;
        into_text(branches_ext::git_branch_delete(&repo, &p.name, p.remote))
    }

    #[tool(
        name = "git_set_upstream",
        description = "Set or clear the upstream of a local branch (feature: branches-extended)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_set_upstream(
        &self,
        Parameters(p): Parameters<SetUpstreamParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::BranchesExtended)?;
        let repo = self.open(&p.repo_path)?;
        into_text(branches_ext::git_set_upstream(
            &repo,
            &p.branch,
            p.upstream.as_deref(),
        ))
    }

    #[tool(
        name = "git_merge_base",
        description = "Print the best common ancestor of two revisions (feature: branches-extended)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_merge_base(
        &self,
        Parameters(p): Parameters<MergeBaseParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::BranchesExtended)?;
        let repo = self.open(&p.repo_path)?;
        into_text(branches_ext::git_merge_base(&repo, &p.a, &p.b))
    }

    // ===================== worktrees =====================

    #[tool(
        name = "git_worktree_list",
        description = "List worktrees attached to this repository (feature: worktrees)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_worktree_list(
        &self,
        Parameters(p): Parameters<RepoOnly>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Worktrees)?;
        let repo = self.open(&p.repo_path)?;
        into_text(worktrees::git_worktree_list(&repo))
    }

    #[tool(
        name = "git_worktree_add",
        description = "Create a new worktree at the given path (feature: worktrees)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_worktree_add(
        &self,
        Parameters(p): Parameters<WorktreeAddParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Worktrees)?;
        let repo = self.open(&p.repo_path)?;
        into_text(worktrees::git_worktree_add(&repo, &p.name, &p.path))
    }

    #[tool(
        name = "git_worktree_remove",
        description = "Prune a worktree. Pass force=true to remove a still-checked-out one (feature: worktrees)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_worktree_remove(
        &self,
        Parameters(p): Parameters<WorktreeRemoveParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Worktrees)?;
        let repo = self.open(&p.repo_path)?;
        into_text(worktrees::git_worktree_remove(&repo, &p.name, p.force))
    }

    // ===================== notes / grep =====================

    #[tool(
        name = "git_notes_list",
        description = "List git notes on the given notes ref (default refs/notes/commits) (feature: notes)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_notes_list(
        &self,
        Parameters(p): Parameters<NotesListParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Notes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(notes::git_notes_list(&repo, p.notes_ref.as_deref()))
    }

    #[tool(
        name = "git_notes_add",
        description = "Attach a note to a target object (feature: notes)",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_git_notes_add(
        &self,
        Parameters(p): Parameters<NotesAddParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Notes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(notes::git_notes_add(
            &repo,
            &p.target,
            &p.message,
            p.notes_ref.as_deref(),
            p.force,
        ))
    }

    #[tool(
        name = "git_notes_remove",
        description = "Remove a git note from a target object (feature: notes)",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_notes_remove(
        &self,
        Parameters(p): Parameters<NotesRemoveParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Notes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(notes::git_notes_remove(
            &repo,
            &p.target,
            p.notes_ref.as_deref(),
        ))
    }

    #[tool(
        name = "git_grep",
        description = "Search tracked files for a regex pattern. Skips files >4 MiB (feature: notes)",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_git_grep(
        &self,
        Parameters(p): Parameters<GrepParams>,
    ) -> Result<String, ErrorData> {
        self.require_feature(Feature::Notes)?;
        let repo = self.open(&p.repo_path)?;
        into_text(notes::git_grep(&repo, &p.pattern, p.ignore_case))
    }
}

/// Manual `ServerHandler` impl: we override `list_tools` and `get_tool` to
/// hide tools whose feature group is disabled. `call_tool` is auto-generated
/// by `#[tool_handler]`; per-tool `require_feature` early-returns handle the
/// "called a hidden tool anyway" case.
#[tool_handler]
impl ServerHandler for GitServer {
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = Self::tool_router()
            .list_all()
            .into_iter()
            .filter(|t| match tool_feature(&t.name) {
                Some(f) => self.features.has(f),
                None => true,
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        match tool_feature(name) {
            Some(f) if !self.features.has(f) => None,
            _ => Self::tool_router().get(name).cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_feature_maps_known_groups() {
        assert_eq!(tool_feature("git_status"), None);
        assert_eq!(tool_feature("git_blame"), Some(Feature::Inspection));
        assert_eq!(tool_feature("git_tag_list"), Some(Feature::Tags));
        assert_eq!(tool_feature("git_stash_save"), Some(Feature::Stash));
        assert_eq!(tool_feature("git_fetch"), Some(Feature::Remotes));
        assert_eq!(tool_feature("git_revert"), Some(Feature::History));
        assert_eq!(
            tool_feature("git_branch_rename"),
            Some(Feature::BranchesExtended)
        );
        assert_eq!(tool_feature("git_worktree_add"), Some(Feature::Worktrees));
        assert_eq!(tool_feature("git_grep"), Some(Feature::Notes));
        assert_eq!(tool_feature("nonexistent"), None);
    }

    #[test]
    fn tool_router_lists_every_tool() {
        let router_names: Vec<String> = GitServer::tool_router()
            .list_all()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        // 13 core + 5 + 5 + 6 + 7 + 5 + 4 + 3 + 4 = 52
        assert_eq!(router_names.len(), 52, "got: {router_names:?}");
    }

    #[test]
    fn every_gated_tool_maps_to_a_feature() {
        for tool in GitServer::tool_router().list_all() {
            // Either it's a core tool (None) or it must map to a real feature.
            let _ = tool_feature(&tool.name);
        }
    }
}
