mod common;

use common::Fixture;
use mcp_server_git_rs::notes::{git_grep, git_notes_add, git_notes_list, git_notes_remove};

#[test]
fn notes_add_list_remove() {
    let f = Fixture::new();
    let added = git_notes_add(&f.repo, "HEAD", "review please", None, false).unwrap();
    assert!(added.contains("Added note"), "got: {added}");

    let listing = git_notes_list(&f.repo, None).unwrap();
    assert!(listing.contains("review please"), "got: {listing}");

    let removed = git_notes_remove(&f.repo, "HEAD", None).unwrap();
    assert!(removed.contains("Removed note"), "got: {removed}");

    let listing = git_notes_list(&f.repo, None).unwrap();
    assert!(!listing.contains("review please"), "got: {listing}");
}

#[test]
fn notes_add_rejects_flag_target() {
    let f = Fixture::new();
    let err = git_notes_add(&f.repo, "--bad", "msg", None, false).unwrap_err();
    assert!(format!("{err}").contains("--"), "got: {err}");
}

#[test]
fn grep_finds_matches_in_tracked_files() {
    let mut f = Fixture::new();
    f.write_commit("hello.txt", "hello world\nbye world\n", "add hello");
    let out = git_grep(&f.repo, "hello", false).unwrap();
    assert!(out.contains("hello.txt"), "got: {out}");
    assert!(out.contains("hello world"), "got: {out}");
    assert!(!out.contains("bye world"), "got: {out}");
}

#[test]
fn grep_case_insensitive() {
    let mut f = Fixture::new();
    f.write_commit("hello.txt", "Hello World\n", "add hello");
    let out = git_grep(&f.repo, "hello", true).unwrap();
    assert!(out.contains("Hello World"), "got: {out}");
}

#[test]
fn grep_skips_untracked() {
    let mut f = Fixture::new();
    f.write_commit("tracked.txt", "needle\n", "tracked");
    f.write("untracked.txt", "needle\n");
    let out = git_grep(&f.repo, "needle", false).unwrap();
    assert!(out.contains("tracked.txt"), "got: {out}");
    assert!(!out.contains("untracked.txt"), "got: {out}");
}

#[test]
fn grep_rejects_empty_pattern() {
    let f = Fixture::new();
    let err = git_grep(&f.repo, "", false).unwrap_err();
    assert!(format!("{err}").contains("empty"), "got: {err}");
}
