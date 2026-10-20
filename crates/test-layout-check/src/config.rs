//! The policy file, and the errors a bad one produces.
//!
//! Every failure carries a stable code and the one change that fixes it, because
//! the caller reading this file is as likely to be a program as a person.

use std::fs;
use std::path::Path;

use serde::Deserialize;

/// The policy file looked for beside a project.
pub const DEFAULT_FILE_NAME: &str = "test-layout.toml";

/// The policy a run uses when no file names one: one external test binary per
/// crate, which is the rule this tool exists to state.
pub const BUILT_IN_MAX_EXTERNAL_TEST_BINARIES: usize = 1;

/// The policy as written in the file, before anything is validated.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub layout: Layout,
}

/// The bound itself, and the packages it does not govern.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    /// The external test binaries one crate may carry.
    #[serde(default = "default_max_external_test_binaries")]
    pub max_external_test_binaries: usize,
    /// Paths this rule does not govern, as `/`-separated patterns where `*`
    /// stays inside a path segment and `**` crosses one. A pattern is matched
    /// against a package's manifest path relative to the root, so a nested
    /// checkout of the same repository is exempt by its place and not by its
    /// name — the names of its crates are this project's names.
    #[serde(default)]
    pub exempt: Vec<String>,
}

/// A policy, validated and ready to run.
#[derive(Clone, Debug)]
pub struct Policy {
    pub max_external_test_binaries: usize,
    pub exempt: Vec<String>,
    /// Where the policy came from, for the report to name.
    pub source: String,
}

impl Policy {
    /// The policy a run uses when no file names one.
    pub fn built_in() -> Self {
        Policy {
            max_external_test_binaries: BUILT_IN_MAX_EXTERNAL_TEST_BINARIES,
            exempt: Vec::new(),
            source: format!("the built-in policy (no {DEFAULT_FILE_NAME} found)"),
        }
    }

    /// Whether a package is exempt from the rule, by the path of its manifest
    /// relative to the root.
    pub fn is_exempt(&self, path: &str) -> bool {
        self.exempt
            .iter()
            .any(|pattern| crate::pattern::matches(pattern, path))
    }
}

impl Config {
    /// Reads and parses the configuration at `path`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|error| ConfigError {
            code: "config/unreadable",
            location: Some(path.display().to_string()),
            message: format!("cannot read the configuration: {error}"),
            remedy: "check that the path exists and is a readable TOML file".to_string(),
        })?;
        Self::parse(&text).map_err(|error| error.in_file(path))
    }

    /// Parses configuration text. A syntax error is reported with the position
    /// inside `text` that caused it.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        toml::from_str(text).map_err(|error| ConfigError {
            code: "config/parse-failed",
            location: error
                .span()
                .map(|span| format!("line {}, column {}", line_of(text, span.start), column_of(text, span.start))),
            message: error.message().trim().to_string(),
            remedy:
                "fix the configuration at this position: it is TOML, and only the documented keys are accepted"
                    .to_string(),
        })
    }

    /// Validates the policy and states where it came from.
    pub fn resolve(self, source: String) -> Result<Policy, ConfigError> {
        if self.layout.max_external_test_binaries == 0 {
            return Err(ConfigError::new(
                "config/bound-is-zero",
                "the bound on external test binaries is zero".to_string(),
                "a crate with one integration area still has one binary, so the bound is at least one",
            ));
        }
        Ok(Policy {
            max_external_test_binaries: self.layout.max_external_test_binaries,
            exempt: self.layout.exempt,
            source,
        })
    }
}

/// A configuration problem, with the code a caller can branch on and the change
/// that resolves it.
#[derive(Clone, Debug)]
pub struct ConfigError {
    /// Stable machine-readable code, e.g. `config/parse-failed`.
    pub code: &'static str,
    /// Where the problem was read from.
    pub location: Option<String>,
    pub message: String,
    /// The one change that makes this go away.
    pub remedy: String,
}

impl ConfigError {
    pub fn new(code: &'static str, message: String, remedy: &str) -> Self {
        Self {
            code,
            location: None,
            message,
            remedy: remedy.to_string(),
        }
    }

    /// Prefixes the location with the file the problem was read from.
    fn in_file(mut self, path: &Path) -> Self {
        self.location = Some(match self.location {
            Some(position) => format!("{}:{position}", path.display()),
            None => path.display().to_string(),
        });
        self
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            Some(location) => write!(formatter, "{} [{location}]", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

fn default_max_external_test_binaries() -> usize {
    BUILT_IN_MAX_EXTERNAL_TEST_BINARIES
}

/// The one-based line of `offset` inside `text`.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

/// The one-based column of `offset` inside `text`.
fn column_of(text: &str, offset: usize) -> usize {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    offset.saturating_sub(start) + 1
}
