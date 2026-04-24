use anyhow::{anyhow, bail, Context as _, Result};
use git2::{Config, Cred, CredentialType, PushOptions, RemoteCallbacks, Repository};
use std::cell::RefCell;
use std::rc::Rc;

use crate::guard::reject_flag_arg;

#[derive(Debug, Clone)]
pub struct PushArgs<'a> {
    pub remote: &'a str,
    pub branch: Option<&'a str>,
    pub force: bool,
    pub set_upstream: bool,
}

pub fn git_push(repo: &Repository, args: PushArgs<'_>) -> Result<String> {
    reject_flag_arg("remote", args.remote)?;
    if let Some(b) = args.branch {
        reject_flag_arg("branch", b)?;
    }

    let branch = match args.branch {
        Some(b) => b.to_owned(),
        None => {
            let head = repo.head().context("failed to resolve HEAD")?;
            head.shorthand()
                .ok_or_else(|| anyhow!("detached HEAD: pass branch explicitly"))?
                .to_owned()
        }
    };

    let refspec = if args.force {
        format!("+refs/heads/{b}:refs/heads/{b}", b = branch)
    } else {
        format!("refs/heads/{b}:refs/heads/{b}", b = branch)
    };

    let mut remote = repo
        .find_remote(args.remote)
        .with_context(|| format!("remote {:?} not found", args.remote))?;

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
            .map_err(|e| anyhow!("push to {} failed: {e}", args.remote))?;
    }

    let failures = failures.borrow();
    if !failures.is_empty() {
        let msg = failures
            .iter()
            .map(|(r, m)| format!("{r}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("push rejected: {msg}");
    }

    let mut upstream_note = "";
    if args.set_upstream {
        let mut cfg = repo.config()?;
        cfg.set_str(&format!("branch.{branch}.remote"), args.remote)?;
        cfg.set_str(
            &format!("branch.{branch}.merge"),
            &format!("refs/heads/{branch}"),
        )?;
        upstream_note = "; upstream set";
    }

    Ok(format!("Pushed {branch} → {}{upstream_note}", args.remote))
}

fn auth_callback(
    url: &str,
    username: Option<&str>,
    allowed: CredentialType,
) -> Result<Cred, git2::Error> {
    if allowed.contains(CredentialType::SSH_KEY) {
        if let Ok(cred) = Cred::ssh_key_from_agent(username.unwrap_or("git")) {
            return Ok(cred);
        }
    }
    if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
        if let Ok(cfg) = Config::open_default() {
            if let Ok(cred) = Cred::credential_helper(&cfg, url, username) {
                return Ok(cred);
            }
        }
        if let Ok(token) = std::env::var("MCP_GIT_TOKEN") {
            return Cred::userpass_plaintext("x-access-token", &token);
        }
        return Err(git2::Error::from_str(
            "HTTPS auth failed — configure a git credential helper or set MCP_GIT_TOKEN",
        ));
    }
    if allowed.contains(CredentialType::DEFAULT) {
        return Cred::default();
    }
    Err(git2::Error::from_str("no usable auth method"))
}
