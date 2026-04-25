use std::fmt::Write;

use anyhow::{Context, Result};
use git2::{Direction, FetchOptions, RemoteCallbacks, Repository};

use crate::guard::reject_flag_arg;
use crate::push::auth_callback;

pub fn git_remote_list(repo: &Repository) -> Result<String> {
    let names = repo.remotes().context("failed to list remotes")?;
    let mut out = Vec::new();
    for n in names.iter().flatten() {
        let r = repo
            .find_remote(n)
            .with_context(|| format!("failed to load remote {n:?}"))?;
        let url = r.url().unwrap_or("<no url>");
        out.push(format!("{n}\t{url}"));
    }
    Ok(out.join("\n"))
}

pub fn git_remote_add(repo: &Repository, name: &str, url: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    reject_flag_arg("url", url)?;
    repo.remote(name, url)
        .with_context(|| format!("failed to add remote {name}"))?;
    Ok(format!("Added remote '{name}' → {url}"))
}

pub fn git_remote_remove(repo: &Repository, name: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    repo.remote_delete(name)
        .with_context(|| format!("failed to remove remote {name}"))?;
    Ok(format!("Removed remote '{name}'"))
}

pub fn git_remote_set_url(repo: &Repository, name: &str, url: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    reject_flag_arg("url", url)?;
    repo.remote_set_url(name, url)
        .with_context(|| format!("failed to update url for remote {name}"))?;
    Ok(format!("Set remote '{name}' → {url}"))
}

pub fn git_fetch(repo: &Repository, name: &str, refspecs: &[String]) -> Result<String> {
    reject_flag_arg("name", name)?;
    for r in refspecs {
        reject_flag_arg("refspec", r)?;
    }

    let mut remote = repo
        .find_remote(name)
        .with_context(|| format!("remote {name:?} not found"))?;

    let mut cbs = RemoteCallbacks::new();
    cbs.credentials(auth_callback);
    let mut opts = FetchOptions::new();
    opts.remote_callbacks(cbs);

    let specs: Vec<&str> = refspecs.iter().map(String::as_str).collect();
    if specs.is_empty() {
        remote
            .fetch::<&str>(&[], Some(&mut opts), None)
            .with_context(|| format!("fetch from {name} failed"))?;
    } else {
        remote
            .fetch(&specs, Some(&mut opts), None)
            .with_context(|| format!("fetch from {name} failed"))?;
    }

    let stats = remote.stats();
    Ok(format!(
        "Fetched {name}: {} objects ({} bytes), {} indexed",
        stats.received_objects(),
        stats.received_bytes(),
        stats.indexed_objects()
    ))
}

pub fn git_remote_prune(repo: &Repository, name: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    let mut remote = repo
        .find_remote(name)
        .with_context(|| format!("remote {name:?} not found"))?;
    let mut cbs = RemoteCallbacks::new();
    cbs.credentials(auth_callback);
    remote
        .prune(Some(cbs))
        .with_context(|| format!("prune {name} failed"))?;
    Ok(format!("Pruned stale tracking refs for '{name}'"))
}

pub fn git_ls_remote(repo: &Repository, name: &str) -> Result<String> {
    reject_flag_arg("name", name)?;
    let mut remote = repo
        .find_remote(name)
        .with_context(|| format!("remote {name:?} not found"))?;

    let mut cbs = RemoteCallbacks::new();
    cbs.credentials(auth_callback);
    let conn = remote
        .connect_auth(Direction::Fetch, Some(cbs), None)
        .with_context(|| format!("connect to {name} failed"))?;

    let mut out = String::new();
    for head in conn
        .list()
        .with_context(|| format!("list refs on {name} failed"))?
    {
        writeln!(out, "{}\t{}", head.oid(), head.name()).unwrap();
    }
    Ok(out)
}
