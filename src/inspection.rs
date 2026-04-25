use std::fmt::Write;

use anyhow::{anyhow, bail, Context, Result};
use git2::{BlameOptions, ObjectType, Repository};

use crate::guard::{reject_flag_arg, require_revparse};

const MAX_BLOB_BYTES: usize = 4 * 1024 * 1024;

pub fn git_blame(repo: &Repository, path: &str, rev: Option<&str>) -> Result<String> {
    reject_flag_arg("path", path)?;
    if let Some(r) = rev {
        reject_flag_arg("rev", r)?;
        require_revparse(repo, r)?;
    }
    blame_range(repo, path, rev, None)
}

pub fn git_blame_line(
    repo: &Repository,
    path: &str,
    start_line: u32,
    end_line: u32,
    rev: Option<&str>,
) -> Result<String> {
    reject_flag_arg("path", path)?;
    if let Some(r) = rev {
        reject_flag_arg("rev", r)?;
        require_revparse(repo, r)?;
    }
    if start_line == 0 || end_line == 0 {
        bail!("line numbers are 1-based; got start={start_line}, end={end_line}");
    }
    if start_line > end_line {
        bail!("start_line ({start_line}) must be <= end_line ({end_line})");
    }
    blame_range(repo, path, rev, Some((start_line, end_line)))
}

fn blame_range(
    repo: &Repository,
    path: &str,
    rev: Option<&str>,
    range: Option<(u32, u32)>,
) -> Result<String> {
    let p = std::path::Path::new(path);
    let mut opts = BlameOptions::new();
    if let Some(r) = rev {
        let oid = repo
            .revparse_single(r)
            .with_context(|| format!("revision {r:?} not found"))?
            .peel_to_commit()
            .with_context(|| format!("{r:?} is not a commit"))?
            .id();
        opts.newest_commit(oid);
    }
    if let Some((s, e)) = range {
        opts.min_line(s as usize).max_line(e as usize);
    }

    let blame = repo
        .blame_file(p, Some(&mut opts))
        .with_context(|| format!("failed to blame {path}"))?;

    let head_oid = match rev {
        Some(r) => repo.revparse_single(r)?.peel_to_commit()?.id(),
        None => repo.head()?.peel_to_commit()?.id(),
    };
    let blob = repo
        .find_object(head_oid, Some(ObjectType::Commit))?
        .peel_to_commit()?
        .tree()?
        .get_path(p)
        .with_context(|| format!("path {path:?} not in tree"))?
        .to_object(repo)?
        .into_blob()
        .map_err(|_| anyhow!("{path:?} is not a blob"))?;
    if blob.size() > MAX_BLOB_BYTES {
        bail!("file too large to blame ({} bytes)", blob.size());
    }
    let content = std::str::from_utf8(blob.content())
        .with_context(|| format!("{path} is not valid UTF-8"))?;

    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        let lineno = idx + 1;
        if let Some((s, e)) = range {
            if (lineno as u32) < s || (lineno as u32) > e {
                continue;
            }
        }
        match blame.get_line(lineno) {
            Some(hunk) => {
                let oid = hunk.final_commit_id();
                let sig = hunk.final_signature();
                let name = sig.name().unwrap_or("Unknown");
                let short = format!("{}", oid);
                let short = &short[..short.len().min(8)];
                writeln!(out, "{short} ({name} L{lineno}) {line}").unwrap();
            }
            None => {
                writeln!(out, "???????? (unknown   L{lineno}) {line}").unwrap();
            }
        }
    }
    Ok(out)
}

pub fn git_ls_tree(repo: &Repository, rev: Option<&str>, path: Option<&str>) -> Result<String> {
    let rev = rev.unwrap_or("HEAD");
    reject_flag_arg("rev", rev)?;
    require_revparse(repo, rev)?;
    if let Some(p) = path {
        reject_flag_arg("path", p)?;
    }

    let commit = repo.revparse_single(rev)?.peel_to_commit()?;
    let tree = commit.tree()?;
    let target_tree = match path {
        Some(p) if !p.is_empty() && p != "." => {
            let entry = tree
                .get_path(std::path::Path::new(p))
                .with_context(|| format!("path {p:?} not in tree"))?;
            let obj = entry.to_object(repo)?;
            match obj.into_tree() {
                Ok(t) => t,
                Err(_) => bail!("{p:?} is not a directory in {rev}"),
            }
        }
        _ => tree,
    };

    let mut out = String::new();
    target_tree
        .walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            let name = entry.name().unwrap_or("<invalid>");
            let kind = match entry.kind() {
                Some(ObjectType::Tree) => "tree",
                Some(ObjectType::Blob) => "blob",
                Some(ObjectType::Commit) => "commit",
                _ => "other",
            };
            let mode = entry.filemode();
            let oid = entry.id();
            let _ = writeln!(out, "{mode:06o} {kind} {oid}\t{dir}{name}");
            git2::TreeWalkResult::Ok
        })
        .context("failed to walk tree")?;
    Ok(out)
}

pub fn git_cat_file(repo: &Repository, spec: &str) -> Result<String> {
    reject_flag_arg("spec", spec)?;
    require_revparse(repo, spec)?;
    let obj = repo
        .revparse_single(spec)
        .with_context(|| format!("{spec:?} not found"))?;
    let kind = obj.kind().map(|k| k.str()).unwrap_or("unknown");
    let mut out = String::new();
    writeln!(out, "Object: {} ({kind})", obj.id()).unwrap();
    match obj.kind() {
        Some(ObjectType::Blob) => {
            let blob = obj.peel_to_blob()?;
            writeln!(out, "Size: {}", blob.size()).unwrap();
            if blob.size() > MAX_BLOB_BYTES {
                writeln!(out, "(content omitted: {} bytes)", blob.size()).unwrap();
            } else {
                match std::str::from_utf8(blob.content()) {
                    Ok(s) => out.push_str(s),
                    Err(_) => writeln!(out, "(binary content, {} bytes)", blob.size()).unwrap(),
                }
            }
        }
        Some(ObjectType::Tree) => {
            let tree = obj.peel_to_tree()?;
            for entry in tree.iter() {
                let name = entry.name().unwrap_or("<invalid>");
                let mode = entry.filemode();
                let kind = match entry.kind() {
                    Some(ObjectType::Tree) => "tree",
                    Some(ObjectType::Blob) => "blob",
                    Some(ObjectType::Commit) => "commit",
                    _ => "other",
                };
                writeln!(out, "{mode:06o} {kind} {}\t{name}", entry.id()).unwrap();
            }
        }
        Some(ObjectType::Commit) => {
            let commit = obj.peel_to_commit()?;
            writeln!(out, "tree {}", commit.tree_id()).unwrap();
            for p in commit.parent_ids() {
                writeln!(out, "parent {p}").unwrap();
            }
            let a = commit.author();
            let c = commit.committer();
            writeln!(
                out,
                "author {} <{}> {}",
                a.name().unwrap_or(""),
                a.email().unwrap_or(""),
                a.when().seconds()
            )
            .unwrap();
            writeln!(
                out,
                "committer {} <{}> {}",
                c.name().unwrap_or(""),
                c.email().unwrap_or(""),
                c.when().seconds()
            )
            .unwrap();
            out.push('\n');
            out.push_str(commit.message().unwrap_or(""));
        }
        Some(ObjectType::Tag) => {
            let tag = obj.peel(ObjectType::Tag)?;
            let raw = tag
                .as_tag()
                .ok_or_else(|| anyhow!("not an annotated tag"))?;
            writeln!(out, "object {}", raw.target_id()).unwrap();
            writeln!(out, "tag {}", raw.name().unwrap_or("")).unwrap();
            if let Some(tagger) = raw.tagger() {
                writeln!(
                    out,
                    "tagger {} <{}> {}",
                    tagger.name().unwrap_or(""),
                    tagger.email().unwrap_or(""),
                    tagger.when().seconds()
                )
                .unwrap();
            }
            out.push('\n');
            out.push_str(raw.message().unwrap_or(""));
        }
        _ => {
            writeln!(out, "(unknown object kind)").unwrap();
        }
    }
    Ok(out)
}

pub fn git_show_ref(repo: &Repository) -> Result<String> {
    let mut lines = Vec::new();
    let refs = repo.references().context("failed to list references")?;
    for r in refs {
        let r = r?;
        let name = match r.name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let oid = match r.target() {
            Some(o) => o,
            None => match r.symbolic_target() {
                Some(t) => {
                    lines.push(format!("symbolic {t}\t{name}"));
                    continue;
                }
                None => continue,
            },
        };
        lines.push(format!("{oid} {name}"));
    }
    lines.sort();
    Ok(lines.join("\n"))
}
