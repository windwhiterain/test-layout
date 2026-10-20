//! The `test-layout-check` command.
//!
//! Failures are printed in the same shape a violation is, because a run that
//! cannot start is a result too: a stable code, a location, and the one change
//! that makes it go away.

use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use test_layout_check::config::{Config, ConfigError};
use test_layout_check::fold::{self, Fold};
use test_layout_check::{CensusError, Checker, Policy, report};

/// Nothing violates: the command succeeded.
const EXIT_CLEAN: u8 = 0;
/// Violations were found: the command did its job and the answer is no.
const EXIT_VIOLATIONS: u8 = 1;
/// The command could not run: nothing was checked.
const EXIT_FAILURE: u8 = 2;

const USAGE: &str = "\
Usage: test-layout-check [OPTIONS] [TARGET]

Checks a Cargo workspace's test layout: every crate carries at most one external
test binary, its integration areas being modules under tests/<name>/.

Arguments:
  [TARGET]  Directory to check. Defaults to the current directory.

Options:
      --config <FILE>  Configuration to load. Defaults to test-layout.toml beside
                       TARGET, then in the current directory, then the built-in
                       policy.
      --format <FMT>   Report format: text (default) or json. The json report
                       carries the same facts plus what was checked, for a
                       caller that acts on the result.
      --max-violations <N>
                       How many violations a report prints in full. Default 10;
                       0 prints every one. What is left out is counted and said
                       out loud, never dropped silently.
  -h, --help           Print this help.
  -V, --version        Print the version.

Exit codes:
  0  no violations
  1  violations found
  2  the command could not run, so nothing was checked
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(failure) => {
            report_failure(&failure);
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

fn run() -> Result<ExitCode, Failure> {
    let arguments = match parse_arguments(std::env::args().skip(1))? {
        Invocation::Help => {
            print!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
        Invocation::Version => {
            println!("test-layout-check {}", env!("CARGO_PKG_VERSION"));
            return Ok(ExitCode::SUCCESS);
        }
        Invocation::Run(arguments) => arguments,
    };

    let root = root_of(&arguments.target);
    let policy = match &arguments.config {
        Some(path) => load_policy(path)?,
        None => match test_layout_check::default_config_path(&root) {
            Some(path) => load_policy(&path)?,
            None => Policy::built_in(),
        },
    };

    let outcome = Checker::new(&root, policy.clone())
        .check()
        .map_err(Failure::from)?;
    let folded = Fold::new(&outcome.violations, arguments.max_violations);
    print!(
        "{}",
        match arguments.format {
            Format::Text => report::text(&outcome, &policy, folded.shown(), folded.omitted()),
            Format::Json => report::json(&outcome, &policy, folded.shown(), folded.omitted()),
        }
    );
    // Decided from what the run found, not from what the report printed, so
    // folding a report away can never turn a failing tree green.
    Ok(if outcome.violations.is_empty() {
        ExitCode::from(EXIT_CLEAN)
    } else {
        ExitCode::from(EXIT_VIOLATIONS)
    })
}

/// The policy a named file carries.
fn load_policy(path: &Path) -> Result<Policy, Failure> {
    let config = Config::load(path)?;
    let source = path.display().to_string();
    Ok(config.resolve(source)?)
}

#[derive(Debug)]
struct Arguments {
    target: PathBuf,
    config: Option<PathBuf>,
    format: Format,
    max_violations: usize,
}

#[derive(Clone, Copy, Debug)]
enum Format {
    Text,
    Json,
}

enum Invocation {
    Run(Arguments),
    Help,
    Version,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Invocation, Failure> {
    let mut target: Option<PathBuf> = None;
    let mut config = None;
    let mut format = Format::Text;
    let mut max_violations = fold::DEFAULT_LIMIT;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "--config" => config = Some(PathBuf::from(value_of(&mut arguments, "--config")?)),
            "--format" => {
                format = match value_of(&mut arguments, "--format")?.as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    other => {
                        return Err(Failure::new(
                            "cli/bad-value",
                            format!("`{other}` is not a report format"),
                            "use `text` or `json`",
                        ));
                    }
                };
            }
            "--max-violations" => {
                let value = value_of(&mut arguments, "--max-violations")?;
                max_violations = value.parse().map_err(|_| {
                    Failure::new(
                        "cli/bad-value",
                        format!("`{value}` is not a number of violations"),
                        "pass zero or a whole number, e.g. `--max-violations 50`; zero prints every violation",
                    )
                })?;
            }
            other if other.starts_with('-') => {
                return Err(Failure::new(
                    "cli/unknown-flag",
                    format!("`{other}` is not an option of this command"),
                    "run `test-layout-check --help` for the options it takes",
                ));
            }
            other => {
                if target.is_some() {
                    return Err(Failure::new(
                        "cli/extra-argument",
                        format!("`{other}` is a second target"),
                        "pass one directory, not two",
                    ));
                }
                target = Some(PathBuf::from(other));
            }
        }
    }
    Ok(Invocation::Run(Arguments {
        target: target.unwrap_or_else(|| PathBuf::from(".")),
        config,
        format,
        max_violations,
    }))
}

/// The value that follows a flag, or a failure naming the flag.
fn value_of(
    arguments: &mut Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<String, Failure> {
    arguments.next().ok_or_else(|| {
        Failure::new(
            "cli/missing-value",
            format!("`{flag}` was given no value"),
            &format!("write `{flag} <VALUE>`; run `test-layout-check --help` for what it takes"),
        )
    })
}

/// The directory a run is rooted at: the target itself for a directory.
fn root_of(target: &Path) -> PathBuf {
    if target.is_dir() {
        target.to_path_buf()
    } else {
        target
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }
}

/// A failure that stopped the command before it could check anything.
struct Failure {
    code: &'static str,
    message: String,
    remedy: String,
}

impl Failure {
    fn new(code: &'static str, message: String, remedy: &str) -> Self {
        Self {
            code,
            message,
            remedy: remedy.to_string(),
        }
    }
}

impl From<ConfigError> for Failure {
    fn from(error: ConfigError) -> Self {
        Self {
            code: error.code,
            message: match &error.location {
                Some(location) => format!("{} [{location}]", error.message),
                None => error.message.clone(),
            },
            remedy: error.remedy,
        }
    }
}

impl From<CensusError> for Failure {
    fn from(error: CensusError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            remedy: error.remedy,
        }
    }
}

fn report_failure(failure: &Failure) {
    eprintln!("error[{}]: {}", failure.code, failure.message);
    eprintln!("  help: {}", failure.remedy);
}
