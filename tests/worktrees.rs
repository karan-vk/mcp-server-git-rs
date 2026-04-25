mod common;

use common::Fixture;
use mcp_server_git_rs::worktrees::{git_worktree_add, git_worktree_list, git_worktree_remove};
use tempfile::TempDir;

#[test]
fn worktree_add_list_remove() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "x\n", "second");

    let other = TempDir::new().unwrap();
    let path = other.path().join("wt1");
    let path_str = path.to_str().unwrap();

    let out = git_worktree_add(&f.repo, "wt1", path_str).unwrap();
    assert!(out.contains("wt1"), "got: {out}");

    let listing = git_worktree_list(&f.repo).unwrap();
    assert!(listing.contains("wt1"), "got: {listing}");

    let removed = git_worktree_remove(&f.repo, "wt1", true).unwrap();
    assert!(removed.contains("wt1"), "got: {removed}");
}

#[test]
fn worktree_add_rejects_flag_name() {
    let f = Fixture::new();
    let dir = TempDir::new().unwrap();
    let err = git_worktree_add(&f.repo, "--bad", dir.path().to_str().unwrap()).unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}
