//! The census: what Cargo will build, read from Cargo itself.
//!
//! `cargo metadata` is the authority, because it names the targets Cargo will
//! build rather than the shape of the working tree. A directory named `main.rs`
//! under `tests/` is a second binary the filesystem does not show, and a file
//! that looks like an area may be none at all.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// One crate in the census.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    /// The directory holding the crate's manifest.
    pub manifest_dir: PathBuf,
    /// The external test binaries: the targets of kind `test`, which are the
    /// ones Cargo links from `tests/*.rs` and `tests/<dir>/main.rs`.
    pub external_test_binaries: Vec<String>,
    /// The other test executables `cargo test` builds for the crate: its
    /// library's own inline harness and the harness of each binary.
    pub inline_test_targets: Vec<String>,
}

impl Package {
    /// How many test executables `cargo test` builds for this crate.
    pub fn test_executables(&self) -> usize {
        self.external_test_binaries.len() + self.inline_test_targets.len()
    }
}

/// Everything the census found.
#[derive(Clone, Debug, Default)]
pub struct Census {
    pub packages: Vec<Package>,
    /// The workspaces the census read, by the directory holding their manifest.
    pub workspaces: Vec<PathBuf>,
}

/// A census that could not be read.
#[derive(Clone, Debug)]
pub struct CensusError {
    pub code: &'static str,
    pub message: String,
    pub remedy: String,
}

impl std::fmt::Display for CensusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CensusError {}

/// The directories never walked when looking for manifests.
const SKIPPED_DIRECTORIES: [&str; 4] = ["target", ".git", ".worktrees", "node_modules"];

/// Reads the census of every workspace under `root`.
///
/// The root's own workspace is read once, and a manifest the root's workspace
/// does not own — a crate excluded from it, or a workspace of its own — is read
/// from its own directory, so a nested workspace is not missed.
pub fn read(root: &Path) -> Result<Census, CensusError> {
    let root = canonical(root);
    let mut census = Census::default();
    let mut covered: BTreeSet<PathBuf> = BTreeSet::new();

    let packages = metadata_in(&root)?;
    census.workspaces.push(root.clone());
    for package in &packages {
        covered.insert(package.manifest_dir.clone());
    }
    census.packages.extend(packages);

    for manifest in manifests_under(&root)? {
        let directory = manifest
            .parent()
            .map_or_else(|| root.clone(), Path::to_path_buf);
        let directory = canonical(&directory);
        if covered.contains(&directory) {
            continue;
        }
        let packages = metadata_in(&directory)?;
        census.workspaces.push(directory.clone());
        for package in &packages {
            covered.insert(package.manifest_dir.clone());
        }
        census.packages.extend(packages);
    }

    census
        .packages
        .sort_by(|left, right| left.name.cmp(&right.name));
    census
        .packages
        .dedup_by(|left, right| left.manifest_dir == right.manifest_dir);
    Ok(census)
}

/// The path as the filesystem names it, so two spellings of one directory
/// compare equal: Cargo answers with absolute paths, and a walk produces the
/// ones it started from.
pub fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Runs `cargo metadata` in `directory` and reads the packages it names.
fn metadata_in(directory: &Path) -> Result<Vec<Package>, CensusError> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(&cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(directory)
        .output()
        .map_err(|error| CensusError {
            code: "census/cargo-not-run",
            message: format!(
                "cannot run `{cargo} metadata` in {}: {error}",
                directory.display()
            ),
            remedy: "check that cargo is on the path and that the directory is a Cargo package"
                .to_string(),
        })?;
    if !output.status.success() {
        return Err(CensusError {
            code: "census/cargo-failed",
            message: format!(
                "`{cargo} metadata` failed in {}: {}",
                directory.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            remedy: "run `cargo metadata` there and read what it says".to_string(),
        });
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|error| CensusError {
        code: "census/metadata-not-json",
        message: format!("`{cargo} metadata` did not answer with json: {error}"),
        remedy: "check that the cargo on the path speaks `--format-version 1`".to_string(),
    })?;
    Ok(read_packages(&value))
}

/// The packages a metadata answer names.
fn read_packages(value: &Value) -> Vec<Package> {
    value["packages"]
        .as_array()
        .map(|packages| packages.iter().filter_map(read_package).collect())
        .unwrap_or_default()
}

/// One package, read from a metadata answer.
fn read_package(value: &Value) -> Option<Package> {
    let name = value["name"].as_str()?.to_string();
    let version = value["version"].as_str().unwrap_or_default().to_string();
    let manifest_path = PathBuf::from(value["manifest_path"].as_str()?);
    let manifest_dir = canonical(manifest_path.parent().unwrap_or(Path::new(".")));

    let mut external_test_binaries = Vec::new();
    let mut inline_test_targets = Vec::new();
    for target in value["targets"].as_array()? {
        let target_name = target["name"].as_str()?.to_string();
        let kinds = target["kind"].as_array()?;
        if kinds.iter().any(|kind| kind == "test") {
            external_test_binaries.push(target_name);
        } else if kinds.iter().any(|kind| kind == "lib" || kind == "bin")
            && target["test"].as_bool().unwrap_or(false)
        {
            inline_test_targets.push(target_name);
        }
    }
    external_test_binaries.sort();
    inline_test_targets.sort();

    Some(Package {
        name,
        version,
        manifest_dir,
        external_test_binaries,
        inline_test_targets,
    })
}

/// Every manifest under `root`, as the directory holding it.
fn manifests_under(root: &Path) -> Result<Vec<PathBuf>, CensusError> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                if directory == root {
                    return Err(CensusError {
                        code: "census/root-unreadable",
                        message: format!("cannot read {}: {error}", directory.display()),
                        remedy: "check that the path exists and is a directory".to_string(),
                    });
                }
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIPPED_DIRECTORIES.contains(&name.as_ref()) {
                continue;
            }
            if path.join("Cargo.toml").is_file() {
                found.push(path.join("Cargo.toml"));
            }
            pending.push(path);
        }
    }
    found.sort();
    Ok(found)
}
