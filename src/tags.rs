use std::cell::RefCell;
use std::rc::Rc;

use anyhow::{anyhow, bail, Context, Result};
use git2::{DescribeFormatOptions, DescribeOptions, PushOptions, RemoteCallbacks, Repository};

use crate::guard::{reject_flag_arg, require_revparse};
use crate::push::auth_callback;

pub fn git_tag_list(repo: &Repository, pattern: Option<&str>) -> Result<String> {
    if let Some(p) = pattern {
        reject_flag_arg("pattern", p)?;
    }
    let names = repo.tag_names(pattern).context("failed to list tags")?;
    let mut out: Vec<String> = names.iter().filter_map(|n| n.map(str::to_owned)).collect();
    out.sort();
    Ok(out.join("\n"))
}

pub fn git_tag_create(
    repo: &Repository,
    name: &str,
    target: Option<&str>,
    message: Option<&str>,
    force: bool,
) -> Result<String> {
    reject_flag_arg("name", name)?;
    let target_spec = target.unwrap_or("HEAD");
    reject_flag_arg("target", target_spec)?;
    require_revparse(repo, target_spec)?;

    let obj = repo
        .revparse_single(target_spec)
        .with_context(|| format!("target {target_spec:?} not found"))?;

    let oid = match message {
        Some(msg) => {
            let sig = repo
                .signature()
                .context("no signature configured — set user.name and user.email")?;
            repo.tag(name, &obj, &sig, msg, force)
                .with_context(|| format!("failed to create tag {name}"))?
        }
        None => repo
            .tag_lightweight(name, &obj, force)
            .with_context(|| format!("failed to create lightweight tag {name}"))?,
    };
    Ok(format!("Created tag '{name}' at {oid}"))
}

pub fn git_tag_delete(repo: &Repository, name: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    repo.tag_delete(name)
        .with_context(|| format!("failed to delete tag {name}"))?;
    Ok(format!("Deleted tag '{name}'"))
}

pub fn git_tag_push(
    repo: &Repository,
    remote_name: &str,
    tag: &str,
    force: bool,
) -> Result<String> {
    reject_flag_arg("remote", remote_name)?;
    reject_flag_arg("tag", tag)?;

    let mut remote = repo
        .find_remote(remote_name)
        .with_context(|| format!("remote {remote_name:?} not found"))?;

    let refspec = if force {
        format!("+refs/tags/{tag}:refs/tags/{tag}")
    } else {
        format!("refs/tags/{tag}:refs/tags/{tag}")
    };

    let failures: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let failures = Rc::clone(&failures);
        let mut cbs = RemoteCallbacks::new();
        cbs.credentials(auth_callback);
        cbs.push_update_reference(move |refname, status| {
            if let Some(msg) = status {
                failures
                    .borrow_mut()
                    .push((refname.to_owned(), msg.to_owned()));
            }
            Ok(())
        });
        let mut opts = PushOptions::new();
        opts.remote_callbacks(cbs);
        remote
            .push(&[refspec.as_str()], Some(&mut opts))
            .map_err(|e| anyhow!("push {tag} to {remote_name} failed: {e}"))?;
    }

    let failures = failures.borrow();
    if !failures.is_empty() {
        let msg = failures
            .iter()
            .map(|(r, m)| format!("{r}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("tag push rejected: {msg}");
    }

    Ok(format!("Pushed tag {tag} → {remote_name}"))
}

pub fn git_describe(
    repo: &Repository,
    rev: Option<&str>,
    tags: bool,
    abbrev: Option<u32>,
) -> Result<String> {
    let mut desc_opts = DescribeOptions::new();
    if tags {
        desc_opts.describe_tags();
    } else {
        desc_opts.describe_all();
    }

    let mut fmt_opts = DescribeFormatOptions::new();
    if let Some(n) = abbrev {
        fmt_opts.abbreviated_size(n);
    }

    let formatted = match rev {
        Some(r) => {
            reject_flag_arg("rev", r)?;
            require_revparse(repo, r)?;
            let obj = repo
                .revparse_single(r)
                .with_context(|| format!("{r:?} not found"))?;
            let describe = obj
                .describe(&desc_opts)
                .with_context(|| format!("describe {r} failed"))?;
            describe
                .format(Some(&fmt_opts))
                .with_context(|| format!("format describe {r} failed"))?
        }
        None => {
            let describe = repo.describe(&desc_opts).context("describe HEAD failed")?;
            describe
                .format(Some(&fmt_opts))
                .context("format describe HEAD failed")?
        }
    };

    Ok(formatted)
}
