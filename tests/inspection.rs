mod common;

use common::Fixture;
use mcp_server_git_rs::inspection::{
    git_blame, git_blame_line, git_cat_file, git_ls_tree, git_show_ref,
};

#[test]
fn blame_full_file() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "alpha\nbeta\n", "add a");
    let out = git_blame(&f.repo, "a.txt", None).unwrap();
    assert!(out.contains("alpha"));
    assert!(out.contains("beta"));
}

#[test]
fn blame_specific_lines() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "alpha\nbeta\ngamma\n", "add a");
    let out = git_blame_line(&f.repo, "a.txt", 2, 2, None).unwrap();
    assert!(out.contains("beta"), "got: {out}");
    assert!(!out.contains("alpha"), "got: {out}");
    assert!(!out.contains("gamma"), "got: {out}");
}

#[test]
fn blame_rejects_flag_path() {
    let f = Fixture::new();
    let err = git_blame(&f.repo, "--malicious", None).unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn blame_line_rejects_zero() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "alpha\n", "add a");
    let err = git_blame_line(&f.repo, "a.txt", 0, 1, None).unwrap_err();
    assert!(format!("{err}").contains("1-based"), "got: {err}");
}

#[test]
fn ls_tree_lists_root() {
    let mut f = Fixture::new();
    f.write_commit("dir/inside.txt", "x\n", "add dir/inside");
    let out = git_ls_tree(&f.repo, None, None).unwrap();
    assert!(out.contains("inside.txt"), "got: {out}");
    assert!(out.contains("blob"), "got: {out}");
}

#[test]
fn ls_tree_subpath() {
    let mut f = Fixture::new();
    f.write_commit("dir/inside.txt", "x\n", "add dir/inside");
    let out = git_ls_tree(&f.repo, None, Some("dir")).unwrap();
    assert!(out.contains("inside.txt"), "got: {out}");
}

#[test]
fn cat_file_blob() {
    let mut f = Fixture::new();
    f.write_commit("hello.txt", "world\n", "add hello");
    let out = git_cat_file(&f.repo, "HEAD:hello.txt").unwrap();
    assert!(out.contains("world"), "got: {out}");
    assert!(out.contains("blob"), "got: {out}");
}

#[test]
fn cat_file_commit() {
    let f = Fixture::new();
    let out = git_cat_file(&f.repo, "HEAD").unwrap();
    assert!(out.contains("commit"), "got: {out}");
    assert!(out.contains("initial commit"), "got: {out}");
}

#[test]
fn show_ref_lists_head_branch() {
    let f = Fixture::new();
    let out = git_show_ref(&f.repo).unwrap();
    assert!(
        out.contains("refs/heads/main") || out.contains("refs/heads/master"),
        "got: {out}"
    );
}
