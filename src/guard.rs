use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn reject_flag_arg(name: &str, val: &str) -> Result<()> {
    if val.starts_with('-') {
        bail!("{name} must not start with '-': {val:?}");
    }
    Ok(())
}

pub fn validate_repo_path(repo_path: &str, allowed_root: Option<&Path>) -> Result<PathBuf> {
    let candidate = PathBuf::from(repo_path);
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("repo_path does not exist or is inaccessible: {repo_path}"))?;

    if let Some(root) = allowed_root {
        let allowed = root
            .canonicalize()
            .with_context(|| format!("allowed root is inaccessible: {}", root.display()))?;
        if !canonical.starts_with(&allowed) {
            return Err(anyhow!(
                "repo_path {} is outside the allowed repository {}",
                canonical.display(),
                allowed.display()
            ));
        }
    }

    Ok(canonical)
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
        assert!(validate_repo_path(path, None).is_ok());
    }

    #[test]
    fn validate_exact_match_allowed() {
        let tmp = TempDir::new().unwrap();
        let canonical = tmp.path().canonicalize().unwrap();
        let result = validate_repo_path(canonical.to_str().unwrap(), Some(&canonical)).unwrap();
        assert_eq!(result, canonical);
    }

    #[test]
    fn validate_subdirectory_allowed() {
        let tmp = TempDir::new().unwrap();
        let canonical_root = tmp.path().canonicalize().unwrap();
        let sub = canonical_root.join("sub");
        fs::create_dir(&sub).unwrap();
        let result = validate_repo_path(sub.to_str().unwrap(), Some(&canonical_root)).unwrap();
        assert!(result.starts_with(&canonical_root));
    }

    #[test]
    fn validate_outside_allowed_rejected() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let allowed_canonical = allowed.path().canonicalize().unwrap();
        let err = validate_repo_path(outside.path().to_str().unwrap(), Some(&allowed_canonical));
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
        let _ = validate_repo_path(&traversal, Some(&allowed_canonical)).ok();
    }

    #[test]
    #[cfg(unix)]
    fn validate_symlink_escape_rejected() {
        let allowed = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let link = allowed.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let allowed_canonical = allowed.path().canonicalize().unwrap();
        let err = validate_repo_path(link.to_str().unwrap(), Some(&allowed_canonical));
        assert!(err.is_err(), "symlink escape should be rejected");
    }
}
