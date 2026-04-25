mod common;

use common::Fixture;
use git2::BranchType;
use mcp_server_git_rs::branches_ext::{
    git_branch_delete, git_branch_rename, git_merge_base, git_set_upstream,
};
use mcp_server_git_rs::tools::git_create_branch;

#[test]
fn branch_rename_changes_name() {
    let f = Fixture::new();
    git_create_branch(&f.repo, "feature", None).unwrap();
    git_branch_rename(&f.repo, "feature", "feature-2", false).unwrap();
    assert!(f.repo.find_branch("feature-2", BranchType::Local).is_ok());
    assert!(f.repo.find_branch("feature", BranchType::Local).is_err());
}

#[test]
fn branch_delete_removes_branch() {
    let f = Fixture::new();
    git_create_branch(&f.repo, "doomed", None).unwrap();
    git_branch_delete(&f.repo, "doomed", false).unwrap();
    assert!(f.repo.find_branch("doomed", BranchType::Local).is_err());
}

#[test]
fn branch_delete_rejects_flag() {
    let f = Fixture::new();
    let err = git_branch_delete(&f.repo, "--bad", false).unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn merge_base_finds_common_ancestor() {
    let mut f = Fixture::new();
    let head_name = f.repo.head().unwrap().shorthand().unwrap().to_owned();
    let base = f.repo.head().unwrap().peel_to_commit().unwrap().id();

    git_create_branch(&f.repo, "feature", None).unwrap();
    f.write_commit("a.txt", "main side\n", "main edit");

    let mb = git_merge_base(&f.repo, "feature", &head_name).unwrap();
    assert_eq!(mb.trim(), format!("{base}"));
}

#[test]
fn set_upstream_sets_and_clears() {
    let f = Fixture::new();
    let head_name = f.repo.head().unwrap().shorthand().unwrap().to_owned();

    f.repo
        .remote("origin", "https://example.com/x.git")
        .unwrap();

    {
        let mut cfg = f.repo.config().unwrap();
        cfg.set_str(&format!("branch.{head_name}.remote"), "origin")
            .unwrap();
        cfg.set_str(
            &format!("branch.{head_name}.merge"),
            &format!("refs/heads/{head_name}"),
        )
        .unwrap();
    }

    let out = git_set_upstream(&f.repo, &head_name, None).unwrap();
    assert!(out.contains("Cleared"), "got: {out}");
}
