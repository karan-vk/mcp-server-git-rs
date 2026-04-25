use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, WorktreePruneOptions};

use crate::guard::reject_flag_arg;

pub fn git_worktree_list(repo: &Repository) -> Result<String> {
    let names = repo.worktrees().context("failed to list worktrees")?;
    let mut out = String::new();
    for n in names.iter().flatten() {
        let wt = match repo.find_worktree(n) {
            Ok(w) => w,
            Err(_) => continue,
        };
        writeln!(out, "{n}\t{}", wt.path().display()).unwrap();
    }
    Ok(out)
}

pub fn git_worktree_add(repo: &Repository, name: &str, path: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    reject_flag_arg("path", path)?;
    let p = Path::new(path);
    let wt = repo
        .worktree(name, p, None)
        .with_context(|| format!("failed to create worktree {name} at {path}"))?;
    Ok(format!(
        "Created worktree '{name}' at {}",
        wt.path().display()
    ))
}

pub fn git_worktree_remove(repo: &Repository, name: &str, force: bool) -> Result<String> {
    reject_flag_arg("name", name)?;
    let wt = repo
        .find_worktree(name)
        .with_context(|| format!("worktree {name:?} not found"))?;
    let mut opts = WorktreePruneOptions::new();
    if force {
        opts.valid(true).locked(true).working_tree(true);
    }
    wt.prune(Some(&mut opts))
        .with_context(|| format!("failed to remove worktree {name}"))?;
    Ok(format!("Removed worktree '{name}'"))
}
