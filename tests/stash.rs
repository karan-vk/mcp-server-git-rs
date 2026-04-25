mod common;

use common::Fixture;
use mcp_server_git_rs::stash::{
    git_stash_apply, git_stash_drop, git_stash_list, git_stash_pop, git_stash_save, git_stash_show,
};

#[test]
fn stash_save_pop_round_trip() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");

    git_stash_save(&mut f.repo, Some("WIP"), false, false).unwrap();

    let listing = git_stash_list(&mut f.repo).unwrap();
    assert!(listing.contains("WIP"), "got: {listing}");

    git_stash_pop(&mut f.repo, 0).unwrap();
    let after = git_stash_list(&mut f.repo).unwrap();
    assert!(after.is_empty(), "got: {after}");

    let contents = std::fs::read_to_string(f.path().join("a.txt")).unwrap();
    assert_eq!(contents, "two\n");
}

#[test]
fn stash_save_apply_keeps_entry() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");

    git_stash_save(&mut f.repo, None, false, false).unwrap();
    git_stash_apply(&mut f.repo, 0).unwrap();
    let listing = git_stash_list(&mut f.repo).unwrap();
    assert!(!listing.is_empty(), "stash should still be present");
    git_stash_drop(&mut f.repo, 0).unwrap();
}

#[test]
fn stash_show_renders_diff() {
    let mut f = Fixture::new();
    f.write_commit("a.txt", "one\n", "add a");
    f.write("a.txt", "two\n");
    git_stash_save(&mut f.repo, Some("change"), false, false).unwrap();

    let shown = git_stash_show(&f.repo, 0).unwrap();
    assert!(shown.contains("Stash:"), "got: {shown}");
    assert!(
        shown.contains("-one") && shown.contains("+two"),
        "got: {shown}"
    );
}

#[test]
fn stash_save_no_changes_errors() {
    let mut f = Fixture::new();
    let err = git_stash_save(&mut f.repo, None, false, false).unwrap_err();
    assert!(format!("{err}").contains("save stash"), "got: {err}");
}
