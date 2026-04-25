use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn reject_flag_arg(name: &str, val: &str) -> Result<()> {
    if val.starts_with('-') {
        bail!("{name} must not start with '-': {val:?}");
    }
    Ok(())
}

pub fn validate_repo_path(repo_path: &str, allowed_roots: &[PathBuf]) -> Result<PathBuf> {
    let candidate = PathBuf::from(repo_path);
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("repo_path does not exist or is inaccessible: {repo_path}"))?;

    if allowed_roots.is_empty() {
        return Ok(canonical);
    }

    let canonical_roots: Vec<PathBuf> = allowed_roots
        .iter()
        .map(|r| {
            r.canonicalize()
                .with_context(|| format!("allowed root is inaccessible: {}", r.display()))
        })
        .collect::<Result<_>>()?;

    if canonical_roots.iter().any(|r| canonical.starts_with(r)) {
        return Ok(canonical);
    }

    if let Some(commondir) = worktree_commondir(&canonical) {
        if canonical_roots.iter().any(|r| commondir.starts_with(r)) {
            return Ok(canonical);
        }
    }

    Err(anyhow!(
        "repo_path {} is outside the allowed repositories ({})",
        canonical.display(),
        canonical_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
    ))
}

/// If `path` is a git worktree (or a regular repo), return the canonical
/// path of its `commondir` (the main repo's `.git` directory's parent for
/// linked worktrees, or the repo itself for a regular repo). Returns `None`
/// when the path can't be opened as a git repo.
fn worktree_commondir(path: &Path) -> Option<PathBuf> {
    let repo = git2::Repository::open(path).ok()?;
    let commondir = repo.commondir();
    // commondir points at a `.git` directory (for the main repo) or at
    // `<main>/.git` directly. Walk up to the working tree root.
    let parent = commondir.parent().unwrap_or(commondir);
    parent.canonicalize().ok()
}

pub fn require_revparse(repo: &git2::Repository, spec: &str) -> Result<()> {
    repo.revparse_single(spec)
        .map(|_| ())
        .map_err(|e| anyhow!("revision {spec:?} not found: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn reject_flag_arg_accepts_normal() {
        assert!(reject_flag_arg("x", "main").is_ok());
        assert!(reject_flag_arg("x", "refs/heads/main").is_ok());
    }

    #[test]
    fn reject_flag_arg_rejects_dash() {
        assert!(reject_flag_arg("x", "--output=/tmp/evil").is_err());
        assert!(reject_flag_arg("x", "-rf").is_err());
    }

    #[test]
    fn validate_no_restriction() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_str().unwrap();
        assert!(validate_repo_path(path, &[]).is_ok());
    }

    #[test]
    fn validate_exact_match_allowed() {
        let tmp = TempDir::new().unwrap();
        let canonical = tmp.path().canonicalize().unwrap();
        let roots = vec![canonical.clone()];
        let result = validate_repo_path(canonical.to_str().unwrap(), &roots).unwrap();
        assert_eq!(result, canonical);
    }

    #[test]
    fn validate_subdirectory_allowed() {
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().canonicalize().unwrap();
        let sub = canonical_root.join("sub");
        fs::create_dir(&sub).unwrap();
        let roots = vec![canonical_root.clone()];
        let result = validate_repo_path(sub.to_str().unwrap(), &roots).unwrap();
        assert!(result.starts_with(&canonical_root));
    }

    #[test]
    fn validate_outside_allowed_rejected() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let allowed_canonical = allowed.path().canonicalize().unwrap();
        let roots = vec![allowed_canonical];
        let err = validate_repo_path(outside.path().to_str().unwrap(), &roots);
        assert!(err.is_err());
    }

    #[test]
    fn validate_traversal_rejected() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let allowed_canonical = allowed.path().canonicalize().unwrap();
        let traversal = format!(
            "{}/../{}",
            allowed.path().display(),
            outside.path().file_name().unwrap().to_str().unwrap()
        );
        let roots = vec![allowed_canonical];
        let _ = validate_repo_path(&traversal, &roots).ok();
    }

    #[test]
    #[cfg(unix)]
    fn validate_symlink_escape_rejected() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let link = allowed.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let allowed_canonical = allowed.path().canonicalize().unwrap();
        let roots = vec![allowed_canonical];
        let err = validate_repo_path(link.to_str().unwrap(), &roots);
        assert!(err.is_err(), "symlink escape should be rejected");
    }

    #[test]
    fn validate_multi_root_accepts_either() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        let roots = vec![
            a.path().canonicalize().unwrap(),
            b.path().canonicalize().unwrap(),
        ];
        assert!(validate_repo_path(a.path().to_str().unwrap(), &roots).is_ok());
        assert!(validate_repo_path(b.path().to_str().unwrap(), &roots).is_ok());
    }

    #[test]
    fn validate_multi_root_rejects_third_sibling() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        let c = TempDir::new().unwrap();
        let roots = vec![
            a.path().canonicalize().unwrap(),
            b.path().canonicalize().unwrap(),
        ];
        assert!(validate_repo_path(c.path().to_str().unwrap(), &roots).is_err());
    }

    #[test]
    fn validate_worktree_of_allowed_root_accepted() {
        let parent = TempDir::new().unwrap();
        let main = parent.path().join("main");
        fs::create_dir(&main).unwrap();

        // git init main
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(&main)
            .status()
            .unwrap();
        assert!(ok.success());

        // need a commit before adding a worktree
        fs::write(main.join("seed"), b"x").unwrap();
        for cmd in [
            vec!["config", "user.email", "t@t"],
            vec!["config", "user.name", "t"],
            vec!["add", "."],
            vec!["commit", "-q", "-m", "seed"],
        ] {
            let s = Command::new("git")
                .args(cmd)
                .current_dir(&main)
                .status()
                .unwrap();
            assert!(s.success());
        }

        let wt = parent.path().join("feature");
        let s = Command::new("git")
            .args([
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                wt.to_str().unwrap(),
            ])
            .current_dir(&main)
            .status()
            .unwrap();
        assert!(s.success());

        let main_canonical = main.canonicalize().unwrap();
        let roots = vec![main_canonical];
        let result = validate_repo_path(wt.to_str().unwrap(), &roots);
        assert!(
            result.is_ok(),
            "worktree path of allowed repo should be accepted: {result:?}"
        );
    }

    #[test]
    fn validate_unrelated_repo_at_sibling_path_rejected() {
        let parent = TempDir::new().unwrap();
        let main = parent.path().join("main");
        let other = parent.path().join("other");
        fs::create_dir(&main).unwrap();
        fs::create_dir(&other).unwrap();

        for dir in [&main, &other] {
            let s = Command::new("git")
                .args(["init", "-q"])
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(s.success());
        }

        let main_canonical = main.canonicalize().unwrap();
        let roots = vec![main_canonical];
        let result = validate_repo_path(other.to_str().unwrap(), &roots);
        assert!(
            result.is_err(),
            "unrelated repo at sibling path must be rejected"
        );
    }
}
