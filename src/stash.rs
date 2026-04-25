use std::fmt::Write;

use anyhow::{Context, Result};
use git2::{DiffFormat, DiffLineType, DiffOptions, Repository, StashFlags};

pub fn git_stash_list(repo: &mut Repository) -> Result<String> {
    let mut entries = Vec::new();
    repo.stash_foreach(|idx, msg, oid| {
        entries.push(format!("stash@{{{idx}}}: {oid} {msg}"));
        true
    })
    .context("failed to walk stash list")?;
    Ok(entries.join("\n"))
}

pub fn git_stash_save(
    repo: &mut Repository,
    message: Option<&str>,
    include_untracked: bool,
    keep_index: bool,
) -> Result<String> {
    let sig = repo
        .signature()
        .context("no signature configured — set user.name and user.email")?;
    let mut flags = StashFlags::DEFAULT;
    if include_untracked {
        flags |= StashFlags::INCLUDE_UNTRACKED;
    }
    if keep_index {
        flags |= StashFlags::KEEP_INDEX;
    }
    let oid = repo
        .stash_save2(&sig, message, Some(flags))
        .context("failed to save stash")?;
    Ok(format!("Saved stash {oid}"))
}

pub fn git_stash_pop(repo: &mut Repository, index: usize) -> Result<String> {
    repo.stash_pop(index, None)
        .with_context(|| format!("failed to pop stash@{{{index}}}"))?;
    Ok(format!("Popped stash@{{{index}}}"))
}

pub fn git_stash_apply(repo: &mut Repository, index: usize) -> Result<String> {
    repo.stash_apply(index, None)
        .with_context(|| format!("failed to apply stash@{{{index}}}"))?;
    Ok(format!("Applied stash@{{{index}}}"))
}

pub fn git_stash_drop(repo: &mut Repository, index: usize) -> Result<String> {
    repo.stash_drop(index)
        .with_context(|| format!("failed to drop stash@{{{index}}}"))?;
    Ok(format!("Dropped stash@{{{index}}}"))
}

pub fn git_stash_show(repo: &Repository, index: usize) -> Result<String> {
    let refname = format!("refs/stash@{{{index}}}");
    let stash_commit = match repo.revparse_single(&refname) {
        Ok(o) => o.peel_to_commit()?,
        Err(_) => repo
            .revparse_single(&format!("stash@{{{index}}}"))
            .with_context(|| format!("stash@{{{index}}} not found"))?
            .peel_to_commit()?,
    };

    let parent = if stash_commit.parent_count() == 0 {
        None
    } else {
        Some(stash_commit.parent(0)?.tree()?)
    };
    let new_tree = stash_commit.tree()?;
    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&new_tree), Some(&mut opts))?;

    let mut out = String::new();
    writeln!(out, "Stash: {}", stash_commit.id()).unwrap();
    writeln!(
        out,
        "Message: {}",
        stash_commit.summary().unwrap_or("").trim_end()
    )
    .unwrap();
    out.push('\n');
    diff.print(DiffFormat::Patch, |_d, _h, line| {
        let origin = match line.origin_value() {
            DiffLineType::Addition => "+",
            DiffLineType::Deletion => "-",
            DiffLineType::Context => " ",
            _ => "",
        };
        out.push_str(origin);
        out.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })?;
    Ok(out)
}
