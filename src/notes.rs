use std::fmt::Write;
use std::fs;

use anyhow::{Context, Result};
use git2::{Repository, Status, StatusOptions};
use regex::Regex;

use crate::guard::{reject_flag_arg, require_revparse};

const MAX_GREP_BYTES: u64 = 4 * 1024 * 1024;

pub fn git_notes_list(repo: &Repository, notes_ref: Option<&str>) -> Result<String> {
    if let Some(r) = notes_ref {
        reject_flag_arg("notes_ref", r)?;
    }
    let iter = match repo.notes(notes_ref) {
        Ok(i) => i,
        Err(_) => return Ok(String::new()),
    };
    let mut out = String::new();
    for entry in iter {
        let (note_oid, target_oid) = entry?;
        let note = repo
            .find_note(notes_ref, target_oid)
            .with_context(|| format!("failed to read note {note_oid}"))?;
        let body = note.message().unwrap_or("").trim_end();
        writeln!(out, "Object: {target_oid}").unwrap();
        writeln!(out, "Note: {note_oid}").unwrap();
        writeln!(out, "{body}").unwrap();
        out.push('\n');
    }
    Ok(out)
}

pub fn git_notes_add(
    repo: &Repository,
    target: &str,
    message: &str,
    notes_ref: Option<&str>,
    force: bool,
) -> Result<String> {
    reject_flag_arg("target", target)?;
    if let Some(r) = notes_ref {
        reject_flag_arg("notes_ref", r)?;
    }
    require_revparse(repo, target)?;

    let target_oid = repo.revparse_single(target)?.id();
    let sig = repo
        .signature()
        .context("no signature configured — set user.name and user.email")?;
    let oid = repo
        .note(&sig, &sig, notes_ref, target_oid, message, force)
        .with_context(|| format!("failed to add note to {target}"))?;
    Ok(format!("Added note {oid} on {target_oid}"))
}

pub fn git_notes_remove(
    repo: &Repository,
    target: &str,
    notes_ref: Option<&str>,
) -> Result<String> {
    reject_flag_arg("target", target)?;
    if let Some(r) = notes_ref {
        reject_flag_arg("notes_ref", r)?;
    }
    require_revparse(repo, target)?;

    let target_oid = repo.revparse_single(target)?.id();
    let sig = repo
        .signature()
        .context("no signature configured — set user.name and user.email")?;
    repo.note_delete(target_oid, notes_ref, &sig, &sig)
        .with_context(|| format!("failed to remove note on {target}"))?;
    Ok(format!("Removed note on {target_oid}"))
}

pub fn git_grep(repo: &Repository, pattern: &str, ignore_case: bool) -> Result<String> {
    if pattern.is_empty() {
        anyhow::bail!("pattern must not be empty");
    }
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repository has no workdir"))?
        .to_path_buf();

    let mut builder = regex::RegexBuilder::new(pattern);
    builder.case_insensitive(ignore_case);
    let re: Regex = builder
        .build()
        .with_context(|| format!("invalid regex {pattern:?}"))?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(false)
        .include_ignored(false)
        .include_unmodified(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .context("failed to enumerate tracked files")?;

    let mut out = String::new();
    let mut total_matches = 0usize;
    for entry in statuses.iter() {
        let s = entry.status();
        if s.contains(Status::IGNORED) || s.contains(Status::WT_NEW) {
            continue;
        }
        let path = match entry.path() {
            Some(p) => p,
            None => continue,
        };
        let abs = workdir.join(path);
        let meta = match fs::metadata(&abs) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() || meta.len() > MAX_GREP_BYTES {
            continue;
        }
        let content = match fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(_) => continue, // binary or unreadable
        };
        for (idx, line) in content.lines().enumerate() {
            if re.is_match(line) {
                writeln!(out, "{path}:{}:{line}", idx + 1).unwrap();
                total_matches += 1;
            }
        }
    }
    if total_matches == 0 {
        return Ok(String::new());
    }
    Ok(out)
}
