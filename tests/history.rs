mod common;

use common::Fixture;
use mcp_server_git_rs::history::{
    git_cherry_pick, git_clean, git_reset_hard, git_rev_parse, git_revert,
};

#[test]
fn rev_parse_resolves_head() {
    let f = Fixture::new();
    let oid = git_rev_parse(&f.repo, "HEAD").unwrap();
    assert_eq!(oid.len(), 40, "got: {oid}");
}

#[test]
fn rev_parse_rejects_flag() {
    let f = Fixture::new();
    let err = git_rev_parse(&f.repo, "--bad").unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn reset_hard_moves_head() {
    let mut f = Fixture::new();
    let first = f.repo.head().unwrap().peel_to_commit().unwrap().id();
    f.write_commit("a.txt", "x\n", "second");

    let out = git_reset_hard(&f.repo, &format!("{first}")).unwrap();
    assert!(out.contains("hard reset"), "got: {out}");

    let head = f.repo.head().unwrap().peel_to_commit().unwrap().id();
    assert_eq!(head, first);
}

#[test]
fn revert_creates_index_changes() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "first\n", "add a");
    let second = f.write_commit("a.txt", "second\n", "modify a");

    let out = git_revert(&f.repo, &format!("{second}")).unwrap();
    assert!(out.contains("Reverted"), "got: {out}");
}

#[test]
fn cherry_pick_applies_commit() {
    let mut f = Fixture::new();
    let initial = f.repo.head().unwrap().peel_to_commit().unwrap().id();
    let target = f.write_commit("b.txt", "from-branch\n", "add b");

    git_reset_hard(&f.repo, &format!("{initial}")).unwrap();

    let out = git_cherry_pick(&f.repo, &format!("{target}")).unwrap();
    assert!(out.contains("Cherry-picked"), "got: {out}");
}

#[test]
fn clean_requires_force() {
    let f = Fixture::new();
    f.write("untracked.txt", "drop me\n");
    let err = git_clean(&f.repo, false).unwrap_err();
    assert!(format!("{err}").contains("force=true"), "got: {err}");
}

#[test]
fn clean_removes_untracked() {
    let f = Fixture::new();
    f.write("untracked.txt", "drop me\n");
    let out = git_clean(&f.repo, true).unwrap();
    assert!(out.contains("Removed"), "got: {out}");
    assert!(!f.path().join("untracked.txt").exists());
}
