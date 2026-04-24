#![allow(dead_code)]

use git2::{Repository, Signature};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Fixture {
    pub dir: TempDir,
    pub repo: Repository,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test User").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
            cfg.set_str("commit.gpgsign", "false").unwrap();
        }
        let mut fixture = Self { dir, repo };
        fixture.write_commit("initial.txt", "initial\n", "initial commit");
        fixture
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn path_str(&self) -> &str {
        self.dir.path().to_str().unwrap()
    }

    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.dir.path().join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, contents).unwrap();
        p
    }

    pub fn stage(&self, rel: &str) {
        let mut idx = self.repo.index().unwrap();
        idx.add_path(Path::new(rel)).unwrap();
        idx.write().unwrap();
    }

    pub fn write_commit(&mut self, rel: &str, contents: &str, message: &str) -> git2::Oid {
        self.write(rel, contents);
        self.stage(rel);
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let mut idx = self.repo.index().unwrap();
        let tree_oid = idx.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_oid).unwrap();
        let parents: Vec<git2::Commit> = match self.repo.head() {
            Ok(h) => vec![h.peel_to_commit().unwrap()],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }
}
