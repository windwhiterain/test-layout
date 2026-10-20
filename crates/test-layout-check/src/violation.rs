//! A violation of the layout policy, and the change that makes it go away.

/// A crate that carries more external test binaries than the bound allows.
#[derive(Clone, Debug)]
pub struct Violation {
    /// Stable machine-readable code, e.g. `layout/too-many-test-binaries`.
    pub code: &'static str,
    /// The crate that carries them.
    pub package: String,
    /// Where the crate lives, relative to the root, with `/` between the
    /// segments.
    pub path: String,
    /// How many it carries.
    pub count: usize,
    /// How many it may carry.
    pub bound: usize,
    /// Their names, in the order Cargo lists them.
    pub binaries: Vec<String>,
    /// The one change that makes this go away.
    pub remedy: String,
}
