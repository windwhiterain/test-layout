//! The two report shapes: a text one for a reader, a json one for a caller.

use serde_json::{Value, json};

use crate::{Outcome, Policy, Violation};

/// The report as text, the way a violation is printed by every other checker.
pub fn text(
    outcome: &Outcome,
    policy: &Policy,
    violations: &[Violation],
    omitted: usize,
) -> String {
    let mut report = String::new();
    for violation in violations {
        report.push_str(&format!(
            "error[{}]: {} carries {} external test binaries, and the bound is {}\n",
            violation.code, violation.package, violation.count, violation.bound
        ));
        report.push_str(&format!("  found: {}\n", violation.binaries.join(", ")));
        report.push_str(&format!("  help: {}\n", violation.remedy));
    }
    if omitted > 0 {
        report.push_str(&format!("  and {omitted} more, not printed in full\n"));
    }
    report.push_str(&format!(
        "checked {} packages in {} workspaces against {}: {} violation{}\n",
        outcome.package_count(),
        outcome.workspace_count(),
        policy.source,
        outcome.violations.len(),
        if outcome.violations.len() == 1 {
            ""
        } else {
            "s"
        }
    ));
    report
}

/// The report as json: the same facts, plus what was checked.
pub fn json(
    outcome: &Outcome,
    policy: &Policy,
    violations: &[Violation],
    omitted: usize,
) -> String {
    let packages: Vec<Value> = outcome
        .census
        .packages
        .iter()
        .map(|package| {
            json!({
                "name": package.name,
                "version": package.version,
                "manifestDir": package.manifest_dir,
                "externalTestBinaries": package.external_test_binaries,
                "inlineTestTargets": package.inline_test_targets,
                "testExecutables": package.test_executables(),
            })
        })
        .collect();
    let violations: Vec<Value> = violations
        .iter()
        .map(|violation| {
            json!({
                "code": violation.code,
                "package": violation.package,
                "path": violation.path,
                "count": violation.count,
                "bound": violation.bound,
                "binaries": violation.binaries,
                "remedy": violation.remedy,
            })
        })
        .collect();
    let value = json!({
        "policy": {
            "source": policy.source,
            "maxExternalTestBinaries": policy.max_external_test_binaries,
            "exempt": policy.exempt,
        },
        "summary": {
            "packages": outcome.package_count(),
            "workspaces": outcome.workspace_count(),
            "violations": outcome.violations.len(),
            "omitted": omitted,
        },
        "violations": violations,
        "packages": packages,
    });
    format!("{value}\n")
}
