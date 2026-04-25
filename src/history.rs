use std::fmt::Write;
use std::fs;

use anyhow::{bail, Context, Result};
use git2::{Repository, ResetType, Status, StatusOptions};

use crate::guard::{reject_flag_arg, require_revparse};

pub fn git_revert(repo: &Repository, rev: &str) -> Result<String> {
    reject_flag_arg("rev", rev)?;
    require_revparse(repo, rev)?;
    let commit = repo
        .revparse_single(rev)?
        .peel_to_commit()
        .with_context(|| format!("{rev:?} is not a commit"))?;
    repo.revert(&commit, None)
        .with_context(|| format!("revert {rev} failed"))?;
    Ok(format!("Reverted {} into the index", commit.id()))
}

pub fn git_cherry_pick(repo: &Repository, rev: &str) -> Result<String> {
    reject_flag_arg("rev", rev)?;
    require_revparse(repo, rev)?;
    let commit = repo
        .revparse_single(rev)?
        .peel_to_commit()
        .with_context(|| format!("{rev:?} is not a commit"))?;
    repo.cherrypick(&commit, None)
        .with_context(|| format!("cherry-pick {rev} failed"))?;
    Ok(format!(
        "Cherry-picked {} into the working tree",
        commit.id()
    ))
}

pub fn git_reset_hard(repo: &Repository, rev: &str) -> Result<String> {
    reject_flag_arg("rev", rev)?;
    require_revparse(repo, rev)?;
    let obj = repo
        .revparse_single(rev)
        .with_context(|| format!("{rev:?} not found"))?;
    repo.reset(&obj, ResetType::Hard, None)
        .with_context(|| format!("reset --hard {rev} failed"))?;
    Ok(format!("HEAD is now at {} (hard reset)", obj.id()))
}

pub fn git_clean(repo: &Repository, force: bool) -> Result<String> {
    if !force {
        bail!("git_clean requires force=true to remove untracked files");
    }
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repository has no workdir"))?
        .to_path_buf();
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .context("failed to read repository status")?;

    let mut removed = Vec::new();
    for entry in statuses.iter() {
        if !entry.status().contains(Status::WT_NEW) {
            continue;
        }
        let path = match entry.path() {
            Some(p) => p,
            None => continue,
        };
        let abs = workdir.join(path);
        if abs.is_file() {
            fs::remove_file(&abs).with_context(|| format!("failed to remove {}", abs.display()))?;
            removed.push(path.to_owned());
        } else if abs.is_dir() {
            fs::remove_dir_all(&abs)
                .with_context(|| format!("failed to remove dir {}", abs.display()))?;
            removed.push(format!("{path}/"));
        }
    }

    let mut out = String::new();
    if removed.is_empty() {
        writeln!(out, "Nothing to clean").unwrap();
    } else {
        writeln!(out, "Removed {} entries:", removed.len()).unwrap();
        for r in removed {
            writeln!(out, "\t{r}").unwrap();
        }
    }
    Ok(out)
}

pub fn git_rev_parse(repo: &Repository, spec: &str) -> Result<String> {
    reject_flag_arg("spec", spec)?;
    let obj = repo
        .revparse_single(spec)
        .with_context(|| format!("{spec:?} not found"))?;
    Ok(format!("{}", obj.id()))
}
