//! The test layout a Cargo workspace is held to.
//!
//! A crate carries at most two test binaries: the lib's own inline one, and one
//! external binary holding every integration area. An area is a module under
//! `tests/<name>/`, never a second top-level file, because Cargo links a binary
//! per `tests/*.rs` and per `tests/<dir>/main.rs` and each one statically links
//! the whole stack the crate depends on.
//!
//! The census is Cargo's own, so the rule is checked against what will actually
//! be built rather than against the shape of a working tree.

pub mod census;
pub mod config;
pub mod fold;
pub mod pattern;
pub mod report;
pub mod violation;

use std::path::{Path, PathBuf};

pub use census::{Census, CensusError, Package};
pub use config::{Config, ConfigError, DEFAULT_FILE_NAME, Policy};
pub use violation::Violation;

/// The checker: a root to walk and a policy to hold it to.
#[derive(Clone, Debug)]
pub struct Checker {
    root: PathBuf,
    policy: Policy,
}

impl Checker {
    pub fn new(root: impl Into<PathBuf>, policy: Policy) -> Self {
        Checker {
            // The root is spelled the way the census spells it, so a package's
            // path is read as a path inside the root rather than as an absolute
            // one -- which is what an exemption pattern is written against.
            root: census::canonical(&root.into()),
            policy,
        }
    }

    /// Reads the census and reports every crate above the bound.
    pub fn check(&self) -> Result<Outcome, CensusError> {
        let census = census::read(&self.root)?;
        let mut violations = Vec::new();
        for package in &census.packages {
            let path = relative_to(&self.root, &package.manifest_dir);
            if self.policy.is_exempt(&path) {
                continue;
            }
            let count = package.external_test_binaries.len();
            if count > self.policy.max_external_test_binaries {
                violations.push(Violation {
                    code: "layout/too-many-test-binaries",
                    package: package.name.clone(),
                    path,
                    count,
                    bound: self.policy.max_external_test_binaries,
                    binaries: package.external_test_binaries.clone(),
                    remedy:
                        "gather the areas as modules under tests/<name>/, so the crate carries one binary"
                            .to_string(),
                });
            }
        }
        Ok(Outcome { census, violations })
    }
}

/// A path as the root names it, with `/` between the segments, so a pattern
/// states one place in the tree rather than one spelling of it.
///
/// Both sides are spelled as the filesystem names them, because the census
/// answers with absolute paths and a caller may name the root any way at all:
/// without that, a path is read as absolute and no exemption pattern matches
/// it.
pub fn relative_to(root: &Path, path: &Path) -> String {
    let root = census::canonical(root);
    let path = census::canonical(path);
    let relative = path.strip_prefix(&root).unwrap_or(&path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join("/")
}

/// What a run found.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// Every crate the census read.
    pub census: Census,
    /// Every crate above the bound.
    pub violations: Vec<Violation>,
}

impl Outcome {
    /// How many packages the census read.
    pub fn package_count(&self) -> usize {
        self.census.packages.len()
    }

    /// How many workspaces the census read.
    pub fn workspace_count(&self) -> usize {
        self.census.workspaces.len()
    }
}

/// The configuration to load when `--config` was not given: the one beside the
/// target, then the one in the current directory, so the command works from
/// anywhere inside a project.
pub fn default_config_path(root: &Path) -> Option<PathBuf> {
    let beside_root = root.join(DEFAULT_FILE_NAME);
    if beside_root.is_file() {
        return Some(beside_root);
    }
    let here = PathBuf::from(DEFAULT_FILE_NAME);
    if here.is_file() {
        return Some(here);
    }
    None
}
