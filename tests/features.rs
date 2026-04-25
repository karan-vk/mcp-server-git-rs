use mcp_server_git_rs::features::{Feature, FeatureSet};
use mcp_server_git_rs::server::{tool_feature, GitServer};

fn visible_count(features: FeatureSet) -> usize {
    GitServer::tool_router()
        .list_all()
        .into_iter()
        .filter(|t| match tool_feature(&t.name) {
            Some(f) => features.has(f),
            None => true,
        })
        .count()
}

fn visible_names(features: FeatureSet) -> Vec<String> {
    GitServer::tool_router()
        .list_all()
        .into_iter()
        .filter(|t| match tool_feature(&t.name) {
            Some(f) => features.has(f),
            None => true,
        })
        .map(|t| t.name.to_string())
        .collect()
}

#[test]
fn empty_set_exposes_only_core() {
    let count = visible_count(FeatureSet::empty());
    assert_eq!(count, 13, "core tool count drifted");
}

#[test]
fn all_set_exposes_every_tool() {
    let count = visible_count(FeatureSet::all());
    assert_eq!(count, 52, "expected 13 core + 39 gated");
}

#[test]
fn enabling_inspection_adds_only_inspection_tools() {
    let mut s = FeatureSet::empty();
    s.enable(Feature::Inspection);
    let names = visible_names(s);
    assert_eq!(names.len(), 13 + 5);
    assert!(names.iter().any(|n| n == "git_blame"));
    assert!(names.iter().any(|n| n == "git_status")); // core
    assert!(!names.iter().any(|n| n == "git_stash_save")); // not enabled
}

#[test]
fn enabling_stash_does_not_leak_other_groups() {
    let mut s = FeatureSet::empty();
    s.enable(Feature::Stash);
    let names = visible_names(s);
    assert!(names.iter().any(|n| n == "git_stash_save"));
    assert!(!names.iter().any(|n| n == "git_blame"));
    assert!(!names.iter().any(|n| n == "git_fetch"));
}

#[test]
fn cli_parser_unknown_feature_errors() {
    let err = FeatureSet::from_cli(&["bogus".into()]).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("bogus"), "got: {msg}");
    for f in Feature::ALL {
        assert!(msg.contains(f.name()), "missing {} in: {msg}", f.name());
    }
}

#[test]
fn cli_parser_accepts_all_shorthand() {
    let s = FeatureSet::from_cli(&["all".into()]).unwrap();
    for f in Feature::ALL {
        assert!(s.has(f));
    }
}

#[test]
fn cli_parser_comma_split_already_done_by_clap() {
    // Clap with `value_delimiter = ','` produces separate Vec<String> entries.
    let s = FeatureSet::from_cli(&["stash".into(), "tags".into(), "notes".into()]).unwrap();
    assert!(s.has(Feature::Stash));
    assert!(s.has(Feature::Tags));
    assert!(s.has(Feature::Notes));
    assert!(!s.has(Feature::Inspection));
}
