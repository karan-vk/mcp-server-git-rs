mod common;

use common::Fixture;
use git2::Repository;
use mcp_server_git_rs::remotes::{
    git_fetch, git_ls_remote, git_remote_add, git_remote_list, git_remote_remove,
    git_remote_set_url,
};
use tempfile::TempDir;

fn bare_remote() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    Repository::init_bare(dir.path()).unwrap();
    let url = format!("file://{}", dir.path().display());
    (dir, url)
}

#[test]
fn remote_add_list_remove_round_trip() {
    let f = Fixture::new();
    let (_remote_dir, url) = bare_remote();

    git_remote_add(&f.repo, "origin", &url).unwrap();
    let listing = git_remote_list(&f.repo).unwrap();
    assert!(listing.contains("origin"), "got: {listing}");
    assert!(listing.contains(&url), "got: {listing}");

    git_remote_remove(&f.repo, "origin").unwrap();
    let after = git_remote_list(&f.repo).unwrap();
    assert!(!after.contains("origin"), "got: {after}");
}

#[test]
fn remote_set_url_changes_url() {
    let f = Fixture::new();
    let (_remote_dir, url) = bare_remote();
    let (_other_dir, url2) = bare_remote();

    git_remote_add(&f.repo, "origin", &url).unwrap();
    git_remote_set_url(&f.repo, "origin", &url2).unwrap();
    let listing = git_remote_list(&f.repo).unwrap();
    assert!(listing.contains(&url2), "got: {listing}");
    assert!(!listing.contains(&url), "got: {listing}");
}

#[test]
fn remote_add_rejects_flag_name() {
    let f = Fixture::new();
    let err = git_remote_add(&f.repo, "--evil", "https://example.com/x.git").unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn fetch_from_local_bare_remote() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "x\n", "first");

    let (_remote_dir, url) = bare_remote();
    git_remote_add(&f.repo, "origin", &url).unwrap();

    let mut r = f.repo.find_remote("origin").unwrap();
    r.push(
        &["refs/heads/master:refs/heads/master".to_string().as_str()],
        None,
    )
    .or_else(|_| {
        r.push(
            &["refs/heads/main:refs/heads/main".to_string().as_str()],
            None,
        )
    })
    .unwrap();
    drop(r);

    let out = git_fetch(&f.repo, "origin", &[]).unwrap();
    assert!(out.contains("Fetched origin"), "got: {out}");
}

#[test]
fn ls_remote_lists_refs_from_local_bare() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "x\n", "first");

    let (_remote_dir, url) = bare_remote();
    git_remote_add(&f.repo, "origin", &url).unwrap();

    let mut r = f.repo.find_remote("origin").unwrap();
    r.push(
        &["refs/heads/master:refs/heads/master".to_string().as_str()],
        None,
    )
    .or_else(|_| {
        r.push(
            &["refs/heads/main:refs/heads/main".to_string().as_str()],
            None,
        )
    })
    .unwrap();
    drop(r);

    let out = git_ls_remote(&f.repo, "origin").unwrap();
    assert!(out.contains("refs/heads/"), "got: {out}");
}
