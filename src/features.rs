use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Inspection,
    Tags,
    Stash,
    Remotes,
    History,
    BranchesExtended,
    Worktrees,
    Notes,
}

impl Feature {
    pub const ALL: [Feature; 8] = [
        Feature::Inspection,
        Feature::Tags,
        Feature::Stash,
        Feature::Remotes,
        Feature::History,
        Feature::BranchesExtended,
        Feature::Worktrees,
        Feature::Notes,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Feature::Inspection => "inspection",
            Feature::Tags => "tags",
            Feature::Stash => "stash",
            Feature::Remotes => "remotes",
            Feature::History => "history",
            Feature::BranchesExtended => "branches-extended",
            Feature::Worktrees => "worktrees",
            Feature::Notes => "notes",
        }
    }

    pub fn parse(s: &str) -> Option<Feature> {
        match s {
            "inspection" => Some(Feature::Inspection),
            "tags" => Some(Feature::Tags),
            "stash" => Some(Feature::Stash),
            "remotes" => Some(Feature::Remotes),
            "history" => Some(Feature::History),
            "branches-extended" => Some(Feature::BranchesExtended),
            "worktrees" => Some(Feature::Worktrees),
            "notes" => Some(Feature::Notes),
            _ => None,
        }
    }

    fn bit(self) -> u16 {
        1u16 << (self as u8)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FeatureSet {
    mask: u16,
}

impl FeatureSet {
    pub fn empty() -> Self {
        Self { mask: 0 }
    }

    pub fn all() -> Self {
        let mut s = Self::empty();
        for f in Feature::ALL {
            s.enable(f);
        }
        s
    }

    pub fn has(&self, f: Feature) -> bool {
        self.mask & f.bit() != 0
    }

    pub fn enable(&mut self, f: Feature) {
        self.mask |= f.bit();
    }

    /// Parse a list of feature names from CLI input. Accepts canonical names
    /// (see `Feature::name`) and `all`. Empty input → empty set. Unknown name
    /// → error listing valid names.
    pub fn from_cli(values: &[String]) -> Result<Self> {
        let mut set = Self::empty();
        for raw in values {
            let v = raw.trim();
            if v.is_empty() {
                continue;
            }
            if v == "all" {
                return Ok(Self::all());
            }
            match Feature::parse(v) {
                Some(f) => set.enable(f),
                None => {
                    let valid = Feature::ALL
                        .iter()
                        .map(|f| f.name())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(anyhow!("unknown feature {v:?} (valid: {valid}, all)"));
                }
            }
        }
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_default() {
        let s = FeatureSet::default();
        for f in Feature::ALL {
            assert!(!s.has(f));
        }
    }

    #[test]
    fn all_has_every_feature() {
        let s = FeatureSet::all();
        for f in Feature::ALL {
            assert!(s.has(f), "missing {}", f.name());
        }
    }

    #[test]
    fn parse_canonical_names() {
        let s = FeatureSet::from_cli(&["stash".into(), "tags".into()]).unwrap();
        assert!(s.has(Feature::Stash));
        assert!(s.has(Feature::Tags));
        assert!(!s.has(Feature::Inspection));
    }

    #[test]
    fn parse_all_shorthand() {
        let s = FeatureSet::from_cli(&["all".into()]).unwrap();
        for f in Feature::ALL {
            assert!(s.has(f));
        }
    }

    #[test]
    fn parse_unknown_errors() {
        let err = FeatureSet::from_cli(&["bogus".into()]).unwrap_err();
        assert!(err.to_string().contains("bogus"));
        assert!(err.to_string().contains("inspection"));
    }

    #[test]
    fn parse_empty_strings_skipped() {
        let s = FeatureSet::from_cli(&["".into(), "stash".into(), "  ".into()]).unwrap();
        assert!(s.has(Feature::Stash));
    }
}
