use anyhow::{Context, Result};
use git2::{BranchType, Repository};

use crate::guard::{reject_flag_arg, require_revparse};

pub fn git_branch_rename(
    repo: &Repository,
    old_name: &str,
    new_name: &str,
    force: bool,
) -> Result<String> {
    reject_flag_arg("old_name", old_name)?;
    reject_flag_arg("new_name", new_name)?;
    let mut br = repo
        .find_branch(old_name, BranchType::Local)
        .with_context(|| format!("branch {old_name:?} not found"))?;
    br.rename(new_name, force)
        .with_context(|| format!("rename {old_name} → {new_name} failed"))?;
    Ok(format!("Renamed branch '{old_name}' → '{new_name}'"))
}

pub fn git_branch_delete(repo: &Repository, name: &str, remote: bool) -> Result<String> {
    reject_flag_arg("name", name)?;
    let kind = if remote {
        BranchType::Remote
    } else {
        BranchType::Local
    };
    let mut br = repo
        .find_branch(name, kind)
        .with_context(|| format!("branch {name:?} not found"))?;
    br.delete()
        .with_context(|| format!("delete branch {name} failed"))?;
    Ok(format!("Deleted branch '{name}'"))
}

pub fn git_set_upstream(repo: &Repository, branch: &str, upstream: Option<&str>) -> Result<String> {
    reject_flag_arg("branch", branch)?;
    if let Some(u) = upstream {
        reject_flag_arg("upstream", u)?;
    }
    let mut br = repo
        .find_branch(branch, BranchType::Local)
        .with_context(|| format!("branch {branch:?} not found"))?;
    br.set_upstream(upstream)
        .with_context(|| format!("set_upstream({:?}) failed", upstream))?;
    match upstream {
        Some(u) => Ok(format!("Set upstream of '{branch}' → '{u}'")),
        None => Ok(format!("Cleared upstream of '{branch}'")),
    }
}

pub fn git_merge_base(repo: &Repository, a: &str, b: &str) -> Result<String> {
    reject_flag_arg("a", a)?;
    reject_flag_arg("b", b)?;
    require_revparse(repo, a)?;
    require_revparse(repo, b)?;
    let oid_a = repo.revparse_single(a)?.peel_to_commit()?.id();
    let oid_b = repo.revparse_single(b)?.peel_to_commit()?.id();
    let base = repo
        .merge_base(oid_a, oid_b)
        .with_context(|| format!("no merge base between {a} and {b}"))?;
    Ok(format!("{base}"))
}
