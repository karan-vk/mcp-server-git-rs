use anyhow::{anyhow, bail, Context, Result};
use git2::{
    BranchType, DiffFormat, DiffLineType, DiffOptions, IndexAddOption, ObjectType, Repository,
    Status, StatusOptions,
};
use std::fmt::Write;
use std::path::Path;

use crate::guard::{reject_flag_arg, require_revparse};

pub const DEFAULT_CONTEXT_LINES: u32 = 3;

pub fn open_repo(path: &Path) -> Result<Repository> {
    Repository::open(path).with_context(|| format!("not a git repository: {}", path.display()))
}

pub fn git_status(repo: &Repository) -> Result<String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);

    let statuses = repo
        .statuses(Some(&mut opts))
        .context("failed to read repository status")?;

    let head_name = repo
        .head()
        .ok()
        .and_then(|r| r.shorthand().map(str::to_owned))
        .unwrap_or_else(|| "HEAD (unborn)".into());

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for entry in statuses.iter() {
        let s = entry.status();
        let path = entry.path().unwrap_or("<invalid utf-8>").to_string();

        if s.contains(Status::INDEX_NEW) {
            staged.push(format!("\tnew file:   {path}"));
        }
        if s.contains(Status::INDEX_MODIFIED) {
            staged.push(format!("\tmodified:   {path}"));
        }
        if s.contains(Status::INDEX_DELETED) {
            staged.push(format!("\tdeleted:    {path}"));
        }
        if s.contains(Status::INDEX_RENAMED) {
            staged.push(format!("\trenamed:    {path}"));
        }
        if s.contains(Status::INDEX_TYPECHANGE) {
            staged.push(format!("\ttypechange: {path}"));
        }

        if s.contains(Status::WT_MODIFIED) {
            unstaged.push(format!("\tmodified:   {path}"));
        }
        if s.contains(Status::WT_DELETED) {
            unstaged.push(format!("\tdeleted:    {path}"));
        }
        if s.contains(Status::WT_RENAMED) {
            unstaged.push(format!("\trenamed:    {path}"));
        }
        if s.contains(Status::WT_TYPECHANGE) {
            unstaged.push(format!("\ttypechange: {path}"));
        }
        if s.contains(Status::WT_NEW) && !s.contains(Status::INDEX_NEW) {
            untracked.push(format!("\t{path}"));
        }
    }

    let mut out = String::new();
    writeln!(out, "On branch {head_name}").unwrap();

    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() {
        writeln!(out, "\nnothing to commit, working tree clean").unwrap();
        return Ok(out);
    }

    if !staged.is_empty() {
        writeln!(out, "\nChanges to be committed:").unwrap();
        for line in &staged {
            writeln!(out, "{line}").unwrap();
        }
    }
    if !unstaged.is_empty() {
        writeln!(out, "\nChanges not staged for commit:").unwrap();
        for line in &unstaged {
            writeln!(out, "{line}").unwrap();
        }
    }
    if !untracked.is_empty() {
        writeln!(out, "\nUntracked files:").unwrap();
        for line in &untracked {
            writeln!(out, "{line}").unwrap();
        }
    }
    Ok(out)
}

fn format_diff(diff: &git2::Diff<'_>) -> Result<String> {
    let mut out = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = match line.origin_value() {
            DiffLineType::Addition => "+",
            DiffLineType::Deletion => "-",
            DiffLineType::Context => " ",
            _ => "",
        };
        out.push_str(origin);
        out.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })
    .context("failed to format diff")?;
    Ok(out)
}

fn diff_options(context_lines: u32) -> DiffOptions {
    let mut opts = DiffOptions::new();
    opts.context_lines(context_lines);
    opts
}

pub fn git_diff_unstaged(repo: &Repository, context_lines: u32) -> Result<String> {
    let mut opts = diff_options(context_lines);
    let diff = repo
        .diff_index_to_workdir(None, Some(&mut opts))
        .context("failed to compute unstaged diff")?;
    format_diff(&diff)
}

pub fn git_diff_staged(repo: &Repository, context_lines: u32) -> Result<String> {
    let head_tree = match repo.head() {
        Ok(h) => Some(h.peel_to_tree().context("failed to peel HEAD to tree")?),
        Err(_) => None,
    };
    let mut opts = diff_options(context_lines);
    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
        .context("failed to compute staged diff")?;
    format_diff(&diff)
}

pub fn git_diff(repo: &Repository, target: &str, context_lines: u32) -> Result<String> {
    reject_flag_arg("target", target)?;
    require_revparse(repo, target)?;

    let target_commit = repo
        .revparse_single(target)?
        .peel_to_commit()
        .with_context(|| format!("target {target:?} is not a commit-ish"))?;
    let target_tree = target_commit.tree()?;

    let mut opts = diff_options(context_lines);
    let diff = repo
        .diff_tree_to_workdir_with_index(Some(&target_tree), Some(&mut opts))
        .context("failed to compute diff against target")?;
    format_diff(&diff)
}

pub fn git_commit(repo: &Repository, message: &str) -> Result<String> {
    let sig = repo
        .signature()
        .context("no signature configured — set user.name and user.email")?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree().context("failed to write index tree")?;
    let tree = repo.find_tree(tree_oid)?;

    let parents: Vec<git2::Commit> = match repo.head() {
        Ok(h) => vec![h.peel_to_commit()?],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .context("failed to create commit")?;
    Ok(format!("Changes committed successfully with hash {oid}"))
}

pub fn git_add(repo: &Repository, files: &[String]) -> Result<String> {
    let mut index = repo.index()?;
    if files == ["."] {
        index
            .add_all(["."].iter(), IndexAddOption::DEFAULT, None)
            .context("failed to add all files")?;
    } else {
        for f in files {
            if f.starts_with('-') {
                bail!("file path must not start with '-': {f:?}");
            }
            index
                .add_path(Path::new(f))
                .with_context(|| format!("failed to stage {f}"))?;
        }
    }
    index.write()?;
    Ok("Files staged successfully".into())
}

pub fn git_reset(repo: &Repository) -> Result<String> {
    let head = match repo.head() {
        Ok(h) => h.peel_to_commit()?,
        Err(_) => bail!("cannot reset: repository has no HEAD (no commits yet)"),
    };
    let head_tree = head.tree()?;
    let mut index = repo.index()?;
    index.read_tree(&head_tree)?;
    index.write()?;
    Ok("All staged changes reset".into())
}

pub fn git_create_branch(
    repo: &Repository,
    branch_name: &str,
    base_branch: Option<&str>,
) -> Result<String> {
    reject_flag_arg("branch_name", branch_name)?;

    let (base_commit, base_display) = match base_branch {
        Some(b) => {
            reject_flag_arg("base_branch", b)?;
            let obj = repo
                .revparse_single(b)
                .with_context(|| format!("base branch {b:?} not found"))?;
            (obj.peel_to_commit()?, b.to_owned())
        }
        None => {
            let head = repo.head()?;
            let name = head
                .shorthand()
                .map(str::to_owned)
                .unwrap_or_else(|| "HEAD".into());
            (head.peel_to_commit()?, name)
        }
    };

    repo.branch(branch_name, &base_commit, false)
        .with_context(|| format!("failed to create branch {branch_name}"))?;
    Ok(format!(
        "Created branch '{branch_name}' from '{base_display}'"
    ))
}

pub fn git_checkout(repo: &Repository, branch_name: &str) -> Result<String> {
    reject_flag_arg("branch_name", branch_name)?;
    require_revparse(repo, branch_name)?;

    let (obj, reference) = repo
        .revparse_ext(branch_name)
        .with_context(|| format!("failed to resolve {branch_name}"))?;

    repo.checkout_tree(&obj, None)
        .context("failed to check out tree")?;

    match reference {
        Some(gref) => {
            let name = gref
                .name()
                .ok_or_else(|| anyhow!("reference has no name"))?
                .to_owned();
            repo.set_head(&name).context("failed to set HEAD")?;
        }
        None => {
            repo.set_head_detached(obj.id())
                .context("failed to set detached HEAD")?;
        }
    }

    Ok(format!("Switched to branch '{branch_name}'"))
}

pub fn git_show(repo: &Repository, revision: &str) -> Result<String> {
    reject_flag_arg("revision", revision)?;
    let obj = repo
        .revparse_single(revision)
        .with_context(|| format!("revision {revision:?} not found"))?;
    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("{revision:?} is not a commit"))?;

    let author = commit.author();
    let time = commit.time();
    let offset_minutes = time.offset_minutes();
    let formatted_time = format_git_time(time.seconds(), offset_minutes);

    let mut out = String::new();
    writeln!(out, "Commit: {}", commit.id()).unwrap();
    writeln!(
        out,
        "Author: {} <{}>",
        author.name().unwrap_or("Unknown"),
        author.email().unwrap_or("unknown")
    )
    .unwrap();
    writeln!(out, "Date: {formatted_time}").unwrap();
    writeln!(
        out,
        "Message: {}",
        commit.message().unwrap_or("").trim_end()
    )
    .unwrap();

    let new_tree = commit.tree()?;
    let old_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;
    out.push('\n');
    out.push_str(&format_diff(&diff)?);
    Ok(out)
}

pub fn git_branch(
    repo: &Repository,
    branch_type: &str,
    contains: Option<&str>,
    not_contains: Option<&str>,
) -> Result<String> {
    if let Some(c) = contains {
        reject_flag_arg("contains", c)?;
    }
    if let Some(c) = not_contains {
        reject_flag_arg("not_contains", c)?;
    }

    let filter = match branch_type {
        "local" => Some(BranchType::Local),
        "remote" => Some(BranchType::Remote),
        "all" => None,
        other => return Ok(format!("Invalid branch type: {other}")),
    };

    let contains_oid = match contains {
        Some(c) => Some(repo.revparse_single(c)?.peel(ObjectType::Commit)?.id()),
        None => None,
    };
    let not_contains_oid = match not_contains {
        Some(c) => Some(repo.revparse_single(c)?.peel(ObjectType::Commit)?.id()),
        None => None,
    };

    let head_shorthand = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(str::to_owned));

    let mut lines = Vec::new();
    for b in repo.branches(filter)? {
        let (branch, btype) = b?;
        let name = match branch.name()? {
            Some(n) => n.to_owned(),
            None => continue,
        };

        let tip = branch.get().peel_to_commit()?.id();
        if let Some(oid) = contains_oid {
            if tip != oid && !repo.graph_descendant_of(tip, oid)? {
                continue;
            }
        }
        if let Some(oid) = not_contains_oid {
            if tip == oid || repo.graph_descendant_of(tip, oid)? {
                continue;
            }
        }

        let prefix = if matches!(btype, BranchType::Local)
            && head_shorthand.as_deref() == Some(name.as_str())
        {
            "*"
        } else {
            " "
        };
        let display = match btype {
            BranchType::Remote => format!("{prefix} remotes/{name}"),
            BranchType::Local => format!("{prefix} {name}"),
        };
        lines.push(display);
    }

    Ok(lines.join("\n"))
}

pub(crate) fn format_git_time(seconds: i64, offset_minutes: i32) -> String {
    let ts = jiff::Timestamp::from_second(seconds).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let offset = jiff::tz::Offset::from_seconds(offset_minutes.saturating_mul(60))
        .unwrap_or(jiff::tz::Offset::UTC);
    let tz = jiff::tz::TimeZone::fixed(offset);
    let zoned = ts.to_zoned(tz);
    zoned.strftime("%a %b %d %H:%M:%S %Y %z").to_string()
}
