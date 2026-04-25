mod common;

use common::Fixture;
use mcp_server_git_rs::tags::{git_describe, git_tag_create, git_tag_delete, git_tag_list};

#[test]
fn tag_create_and_list() {
    let f = Fixture::new();
    git_tag_create(&f.repo, "v0.1", None, None, false).unwrap();
    let out = git_tag_list(&f.repo, None).unwrap();
    assert!(out.contains("v0.1"), "got: {out}");
}

#[test]
fn tag_create_annotated() {
    let f = Fixture::new();
    let msg = "release notes";
    git_tag_create(&f.repo, "v1.0", None, Some(msg), false).unwrap();
    let out = git_tag_list(&f.repo, None).unwrap();
    assert!(out.contains("v1.0"), "got: {out}");
}

#[test]
fn tag_create_force_overwrites() {
    let mut f = Fixture::new();
    git_tag_create(&f.repo, "v0.1", None, None, false).unwrap();
    f.write_commit("a.txt", "x\n", "another commit");
    git_tag_create(&f.repo, "v0.1", None, None, true).unwrap();
}

#[test]
fn tag_delete_removes_tag() {
    let f = Fixture::new();
    git_tag_create(&f.repo, "v0.2", None, None, false).unwrap();
    git_tag_delete(&f.repo, "v0.2").unwrap();
    let out = git_tag_list(&f.repo, None).unwrap();
    assert!(!out.contains("v0.2"), "got: {out}");
}

#[test]
fn tag_create_rejects_flag_name() {
    let f = Fixture::new();
    let err = git_tag_create(&f.repo, "--bad", None, None, false).unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn tag_list_pattern_filters() {
    let f = Fixture::new();
    git_tag_create(&f.repo, "v0.1", None, None, false).unwrap();
    git_tag_create(&f.repo, "rc-1", None, None, false).unwrap();
    let out = git_tag_list(&f.repo, Some("v*")).unwrap();
    assert!(out.contains("v0.1"), "got: {out}");
    assert!(!out.contains("rc-1"), "got: {out}");
}

#[test]
fn describe_with_tag() {
    let f = Fixture::new();
    git_tag_create(&f.repo, "v0.3", None, None, false).unwrap();
    let out = git_describe(&f.repo, None, true, None).unwrap();
    assert!(out.contains("v0.3"), "got: {out}");
}
