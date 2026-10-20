//! How many violations a report prints in full, and how many it leaves out.
//!
//! What is left out is counted and said out loud, never dropped silently: a
//! report that shows one of forty violations must not read as one violation.

use crate::Violation;

/// How many violations a report prints in full unless told otherwise.
pub const DEFAULT_LIMIT: usize = 10;

/// The violations a report prints, and the ones it counts but does not print.
#[derive(Clone, Debug)]
pub struct Fold<'a> {
    violations: &'a [Violation],
    limit: usize,
}

impl<'a> Fold<'a> {
    pub fn new(violations: &'a [Violation], limit: usize) -> Self {
        Fold { violations, limit }
    }

    /// The violations printed in full.
    pub fn shown(&self) -> &'a [Violation] {
        if self.limit == 0 {
            self.violations
        } else {
            &self.violations[..self.violations.len().min(self.limit)]
        }
    }

    /// How many are counted but not printed.
    pub fn omitted(&self) -> usize {
        self.violations.len() - self.shown().len()
    }
}
