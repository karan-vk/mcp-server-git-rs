mod common;

use common::Fixture;
use git2::Repository;
use mcp_server_git_rs::push::{git_push, PushArgs};
use tempfile::TempDir;

fn setup_bare_remote(f: &Fixture) -> (TempDir, String) {
    let bare_dir = TempDir::new().unwrap();
    Repository::init_bare(bare_dir.path()).unwrap();
    let url = format!("file://{}", bare_dir.path().display());
    f.repo.remote("origin", &url).unwrap();
    (bare_dir, url)
}

fn current_branch(repo: &Repository) -> String {
    repo.head().unwrap().shorthand().unwrap().to_string()
}

#[test]
fn push_to_local_bare_succeeds() {
    let f = Fixture::new();
    let (_bare, _url) = setup_bare_remote(&f);
    let branch = current_branch(&f.repo);
    let out = git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some(&branch),
            force: false,
            set_upstream: false,
        },
    )
    .unwrap();
    assert!(out.starts_with("Pushed"), "got: {out}");
}

#[test]
fn push_set_upstream_writes_config() {
    let f = Fixture::new();
    let (_bare, _url) = setup_bare_remote(&f);
    let branch = current_branch(&f.repo);
    let out = git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some(&branch),
            force: false,
            set_upstream: true,
        },
    )
    .unwrap();
    assert!(out.contains("upstream set"), "got: {out}");

    let cfg = f.repo.config().unwrap();
    let remote = cfg.get_string(&format!("branch.{branch}.remote")).unwrap();
    assert_eq!(remote, "origin");
    let merge = cfg.get_string(&format!("branch.{branch}.merge")).unwrap();
    assert_eq!(merge, format!("refs/heads/{branch}"));
}

#[test]
fn push_rejects_flag_remote() {
    let f = Fixture::new();
    let err = git_push(
        &f.repo,
        PushArgs {
            remote: "--evil",
            branch: None,
            force: false,
            set_upstream: false,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn push_rejects_flag_branch() {
    let f = Fixture::new();
    let err = git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some("--evil"),
            force: false,
            set_upstream: false,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn force_push_rewrites_diverged_remote() {
    let f = Fixture::new();
    let (_bare, _url) = setup_bare_remote(&f);
    let branch = current_branch(&f.repo);

    // First push — establish remote.
    git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some(&branch),
            force: false,
            set_upstream: false,
        },
    )
    .unwrap();

    // Rewrite history: amend the initial commit so the local head diverges.
    let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
    let head = f.repo.head().unwrap().peel_to_commit().unwrap();
    head.amend(
        Some("HEAD"),
        Some(&sig),
        Some(&sig),
        None,
        Some("amended"),
        None,
    )
    .unwrap();

    // Non-force push should fail with a fast-forward rejection.
    let err = git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some(&branch),
            force: false,
            set_upstream: false,
        },
    )
    .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("rejected") || msg.contains("fastforward") || msg.contains("fast-forward"),
        "expected fast-forward rejection, got: {err}"
    );

    // Force push should succeed.
    let out = git_push(
        &f.repo,
        PushArgs {
            remote: "origin",
            branch: Some(&branch),
            force: true,
            set_upstream: false,
        },
    )
    .unwrap();
    assert!(out.starts_with("Pushed"), "got: {out}");
}
