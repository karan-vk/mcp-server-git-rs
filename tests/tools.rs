mod common;

use common::Fixture;
use mcp_server_git_rs::log::git_log;
use mcp_server_git_rs::tools::{
    git_add, git_branch, git_checkout, git_commit, git_create_branch, git_diff, git_diff_staged,
    git_diff_unstaged, git_reset, git_show, git_status,
};
use std::fs;
use std::path::Path;

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

fn in_index(f: &Fixture, rel: &str) -> bool {
    f.repo
        .index()
        .unwrap()
        .get_path(Path::new(rel), 0)
        .is_some()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn add_directory_rename_in_one_call() {
    let mut f = Fixture::new();
    f.write_commit("olddir/a.txt", "a\n", "add a");
    f.write_commit("olddir/sub/b.txt", "b\n", "add b");
    fs::rename(f.path().join("olddir"), f.path().join("newdir")).unwrap();

    let out = git_add(&f.repo, &strings(&["olddir", "newdir"])).unwrap();
    assert_eq!(out, "Files staged successfully");
    assert!(in_index(&f, "newdir/a.txt"));
    assert!(in_index(&f, "newdir/sub/b.txt"));
    assert!(!in_index(&f, "olddir/a.txt"));
    assert!(!in_index(&f, "olddir/sub/b.txt"));
}

#[test]
fn add_directory_recurses() {
    let f = Fixture::new();
    f.write("dir/a.txt", "a\n");
    f.write("dir/nested/b.txt", "b\n");
    git_add(&f.repo, &strings(&["dir"])).unwrap();
    assert!(in_index(&f, "dir/a.txt"));
    assert!(in_index(&f, "dir/nested/b.txt"));
}

#[test]
fn add_deleted_file_stages_removal() {
    let f = Fixture::new();
    fs::remove_file(f.path().join("initial.txt")).unwrap();
    let out = git_add(&f.repo, &strings(&["initial.txt"])).unwrap();
    assert_eq!(out, "Files staged successfully");
    assert!(!in_index(&f, "initial.txt"));
}

#[test]
fn add_glob_pathspec() {
    let f = Fixture::new();
    f.write("a.rs", "a\n");
    f.write("b.rs", "b\n");
    f.write("c.md", "c\n");
    git_add(&f.repo, &strings(&["*.rs"])).unwrap();
    assert!(in_index(&f, "a.rs"));
    assert!(in_index(&f, "b.rs"));
    assert!(!in_index(&f, "c.md"));
}

#[test]
fn add_normalises_path_forms() {
    let f = Fixture::new();
    f.write("dir/a.txt", "a\n");
    f.write("dir2/b.txt", "b\n");
    f.write("c.txt", "c\n");
    let abs = f.path().join("c.txt").to_str().unwrap().to_owned();
    git_add(
        &f.repo,
        &["./dir/a.txt".to_string(), "dir2/".to_string(), abs],
    )
    .unwrap();
    assert!(in_index(&f, "dir/a.txt"));
    assert!(in_index(&f, "dir2/b.txt"));
    assert!(in_index(&f, "c.txt"));
}

#[test]
fn add_rejects_path_outside_repo() {
    let f = Fixture::new();
    for bad in ["../outside.txt", "dir/../../outside.txt", "/etc/passwd"] {
        let err = git_add(&f.repo, &strings(&[bad])).unwrap_err();
        assert!(
            err.to_string().contains("is outside the repository"),
            "{bad}: {err}"
        );
    }
}

#[test]
fn add_unmatched_pathspec_errors_and_stages_nothing() {
    let f = Fixture::new();
    f.write("ok.txt", "ok\n");
    let err = git_add(&f.repo, &strings(&["ok.txt", "nomatch.txt"])).unwrap_err();
    assert_eq!(
        err.to_string(),
        "pathspec 'nomatch.txt' did not match any files"
    );
    assert!(!in_index(&f, "ok.txt"));
}

#[test]
fn add_ignored_path_errors() {
    let f = Fixture::new();
    f.write(".gitignore", "*.log\n");
    f.write("debug.log", "x\n");
    let err = git_add(&f.repo, &strings(&["debug.log"])).unwrap_err();
    assert!(
        err.to_string()
            .contains("ignored by one of your .gitignore files"),
        "got: {err}"
    );
    assert!(!in_index(&f, "debug.log"));
}

#[test]
fn add_empty_list_errors() {
    let f = Fixture::new();
    let err = git_add(&f.repo, &[]).unwrap_err();
    assert!(
        err.to_string().starts_with("No files provided to stage"),
        "got: {err}"
    );
}

#[test]
fn add_nothing_to_stage_reports_no_changes() {
    let f = Fixture::new();
    for files in [&["."][..], &["initial.txt"][..]] {
        let out = git_add(&f.repo, &strings(files)).unwrap();
        assert!(out.starts_with("No changes were staged"), "got: {out}");
    }
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
    assert_eq!(f.repo.head().unwrap().shorthand().unwrap(), "feature");
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
