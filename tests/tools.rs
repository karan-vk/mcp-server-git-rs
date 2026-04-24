mod common;

use common::Fixture;
use mcp_server_git_rs::log::git_log;
use mcp_server_git_rs::tools::{
    git_add, git_branch, git_checkout, git_commit, git_create_branch, git_diff, git_diff_staged,
    git_diff_unstaged, git_reset, git_show, git_status,
};

#[test]
fn status_clean_repo() {
    let f = Fixture::new();
    let out = git_status(&f.repo).unwrap();
    assert!(out.contains("nothing to commit"), "got: {out}");
}

#[test]
fn status_shows_untracked() {
    let f = Fixture::new();
    f.write("new.txt", "hi\n");
    let out = git_status(&f.repo).unwrap();
    assert!(out.contains("Untracked files"), "got: {out}");
    assert!(out.contains("new.txt"), "got: {out}");
}

#[test]
fn status_shows_modified() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");
    let out = git_status(&f.repo).unwrap();
    assert!(out.contains("modified"), "got: {out}");
}

#[test]
fn diff_unstaged_returns_patch() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");
    let out = git_diff_unstaged(&f.repo, 3).unwrap();
    assert!(out.contains("-one") && out.contains("+two"), "got: {out}");
}

#[test]
fn diff_unstaged_empty_when_clean() {
    let f = Fixture::new();
    assert_eq!(git_diff_unstaged(&f.repo, 3).unwrap(), "");
}

#[test]
fn diff_staged_shows_staged() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");
    f.stage("a.txt");
    let out = git_diff_staged(&f.repo, 3).unwrap();
    assert!(out.contains("-one") && out.contains("+two"), "got: {out}");
}

#[test]
fn diff_staged_empty_when_clean() {
    let f = Fixture::new();
    assert_eq!(git_diff_staged(&f.repo, 3).unwrap(), "");
}

#[test]
fn diff_against_target_branch() {
    let mut f = Fixture::new();
    // create a second branch pointing at initial, then diverge on main
    let head_oid = f.repo.head().unwrap().peel_to_commit().unwrap().id();
    f.repo
        .branch("other", &f.repo.find_commit(head_oid).unwrap(), false)
        .unwrap();
    f.write_commit("b.txt", "hello\n", "add b on main");
    let out = git_diff(&f.repo, "other", 3).unwrap();
    assert!(out.contains("b.txt"), "got: {out}");
}

#[test]
fn diff_rejects_flag_injection() {
    let f = Fixture::new();
    let err = git_diff(&f.repo, "--output=/tmp/evil", 3).unwrap_err();
    assert!(
        err.to_string().contains("must not start with '-'"),
        "got: {err}"
    );
}

#[test]
fn diff_rejects_nonexistent_ref() {
    let f = Fixture::new();
    let err = git_diff(&f.repo, "does-not-exist", 3).unwrap_err();
    assert!(err.to_string().contains("not found"), "got: {err}");
}

#[test]
fn commit_creates_commit() {
    let f = Fixture::new();
    f.write("c.txt", "c\n");
    f.stage("c.txt");
    let out = git_commit(&f.repo, "add c").unwrap();
    assert!(
        out.starts_with("Changes committed successfully"),
        "got: {out}"
    );
}

#[test]
fn add_specific_file() {
    let f = Fixture::new();
    f.write("x.txt", "x\n");
    let out = git_add(&f.repo, &["x.txt".to_string()]).unwrap();
    assert_eq!(out, "Files staged successfully");
    assert!(git_diff_staged(&f.repo, 3).unwrap().contains("+x"));
}

#[test]
fn add_all_files_via_dot() {
    let f = Fixture::new();
    f.write("y.txt", "y\n");
    f.write("z.txt", "z\n");
    let out = git_add(&f.repo, &[".".to_string()]).unwrap();
    assert_eq!(out, "Files staged successfully");
    let staged = git_diff_staged(&f.repo, 3).unwrap();
    assert!(
        staged.contains("y.txt") && staged.contains("z.txt"),
        "got: {staged}"
    );
}

#[test]
fn add_rejects_flag_file() {
    let f = Fixture::new();
    let err = git_add(&f.repo, &["--evil".to_string()]).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn reset_unstages() {
    let f = Fixture::new();
    f.write("r.txt", "r\n");
    f.stage("r.txt");
    assert!(!git_diff_staged(&f.repo, 3).unwrap().is_empty());
    let out = git_reset(&f.repo).unwrap();
    assert_eq!(out, "All staged changes reset");
    assert!(git_diff_staged(&f.repo, 3).unwrap().is_empty());
}

#[test]
fn log_default_lists_commits() {
    let f = Fixture::new();
    let out = git_log(&f.repo, 10, None, None).unwrap();
    assert!(out.contains("initial commit"), "got: {out}");
}

#[test]
fn log_max_count_caps_results() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "a\n", "add a");
    f.write_commit("b.txt", "b\n", "add b");
    let out = git_log(&f.repo, 1, None, None).unwrap();
    let count = out.matches("Commit:").count();
    assert_eq!(count, 1, "got: {out}");
}

#[test]
fn log_rejects_flag_in_timestamp() {
    let f = Fixture::new();
    let err = git_log(&f.repo, 10, Some("--since=evil"), None).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn create_branch_from_head() {
    let f = Fixture::new();
    let out = git_create_branch(&f.repo, "feature", None).unwrap();
    assert!(out.contains("Created branch 'feature'"), "got: {out}");
    assert!(f
        .repo
        .find_branch("feature", git2::BranchType::Local)
        .is_ok());
}

#[test]
fn create_branch_rejects_flag_name() {
    let f = Fixture::new();
    let err = git_create_branch(&f.repo, "--evil", None).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn create_branch_rejects_flag_base() {
    let f = Fixture::new();
    let err = git_create_branch(&f.repo, "ok", Some("--evil")).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn checkout_switches_branch() {
    let f = Fixture::new();
    git_create_branch(&f.repo, "feature", None).unwrap();
    let out = git_checkout(&f.repo, "feature").unwrap();
    assert!(out.contains("feature"), "got: {out}");
    assert_eq!(f.repo.head().unwrap().shorthand(), Some("feature"));
}

#[test]
fn checkout_rejects_flag() {
    let f = Fixture::new();
    let err = git_checkout(&f.repo, "--evil").unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn checkout_rejects_missing_ref() {
    let f = Fixture::new();
    let err = git_checkout(&f.repo, "nope").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn show_initial_commit() {
    let f = Fixture::new();
    let head = f
        .repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id()
        .to_string();
    let out = git_show(&f.repo, &head).unwrap();
    assert!(out.contains("initial commit"), "got: {out}");
    assert!(out.contains("initial.txt"), "got: {out}");
}

#[test]
fn show_rejects_flag() {
    let f = Fixture::new();
    let err = git_show(&f.repo, "--evil").unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn branch_local_lists() {
    let f = Fixture::new();
    git_create_branch(&f.repo, "feature", None).unwrap();
    let out = git_branch(&f.repo, "local", None, None).unwrap();
    assert!(out.contains("feature"), "got: {out}");
}

#[test]
fn branch_rejects_flag_contains() {
    let f = Fixture::new();
    let err = git_branch(&f.repo, "local", Some("--evil"), None).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}

#[test]
fn branch_rejects_flag_not_contains() {
    let f = Fixture::new();
    let err = git_branch(&f.repo, "local", None, Some("--evil")).unwrap_err();
    assert!(err.to_string().contains("must not start with '-'"));
}
