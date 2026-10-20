//! The policy, the fold, and the census read from Cargo's own answer.

use test_layout_check::config::Config;
use test_layout_check::fold::Fold;
use test_layout_check::violation::Violation;
use test_layout_check::{Checker, Policy};

/// The policy as a project states it.
const POLICY: &str = r#"
[layout]
max_external_test_binaries = 1
exempt = ["a-crate-the-rule-does-not-govern"]
"#;

fn policy(text: &str) -> Policy {
    let config = Config::parse(text).expect("the test policy parses");
    config
        .resolve("the test policy".to_string())
        .expect("the test policy resolves")
}

fn violation(package: &str) -> Violation {
    Violation {
        code: "layout/too-many-test-binaries",
        package: package.to_string(),
        path: "crates/a-crate".to_string(),
        count: 2,
        bound: 1,
        binaries: vec!["one".to_string(), "two".to_string()],
        remedy: "gather the areas as modules".to_string(),
    }
}

#[test]
fn a_policy_states_the_bound_and_the_exemptions() {
    let policy = policy(POLICY);
    assert_eq!(policy.max_external_test_binaries, 1);
    assert!(policy.is_exempt("a-crate-the-rule-does-not-govern"));
    assert!(!policy.is_exempt("a-crate-the-rule-governs"));
    assert!(!policy.is_exempt("crates/a-crate"));
}

#[test]
fn a_bound_of_zero_is_refused() {
    let error = Config::parse("[layout]\nmax_external_test_binaries = 0\n")
        .expect("the text parses")
        .resolve("the test policy".to_string())
        .expect_err("a bound of zero is refused");
    assert_eq!(error.code, "config/bound-is-zero");
}

#[test]
fn an_unknown_key_is_refused() {
    let error = Config::parse("[layout]\nmax_external_binaries = 1\n")
        .expect_err("the unknown key is refused");
    assert_eq!(error.code, "config/parse-failed");
}

#[test]
fn the_built_in_policy_is_one_external_binary() {
    let policy = Policy::built_in();
    assert_eq!(policy.max_external_test_binaries, 1);
    assert!(policy.exempt.is_empty());
}

#[test]
fn a_folded_report_counts_what_it_leaves_out() {
    let violations = vec![violation("a"), violation("b"), violation("c")];
    let folded = Fold::new(&violations, 2);
    assert_eq!(folded.shown().len(), 2);
    assert_eq!(folded.omitted(), 1);
    let every = Fold::new(&violations, 0);
    assert_eq!(every.shown().len(), 3);
    assert_eq!(every.omitted(), 0);
}

#[test]
fn this_repository_carries_one_external_test_binary() {
    // The repository under test is the tool's own: one crate, one binary.
    let root = env!("CARGO_MANIFEST_DIR");
    let outcome = Checker::new(root, Policy::built_in())
        .check()
        .expect("the census is read");
    assert_eq!(outcome.package_count(), 1);
    assert!(outcome.violations.is_empty(), "{:?}", outcome.violations);
}
