# test-layout

A checker for the test layout of a Cargo workspace: **a crate carries one
external test binary, not one per area**.

Cargo links a test binary per `tests/*.rs` and per `tests/<dir>/main.rs`, and
each one statically links the whole stack its crate depends on. A crate with
twenty areas under `tests/` therefore has twenty binaries and twenty relinks.
The areas belong under `tests/<name>/` as modules of one binary, and the
individual test is then selected by an argument rather than by the file it sits
in.

## Use

```
cargo install --git https://github.com/windwhiterain/test-layout.git test-layout-check --locked
test-layout-check --config test-layout.toml
```

The command reads the census from Cargo itself — `cargo metadata --no-deps` —
so the rule is checked against what will actually be built, not against the
shape of a working tree. A `tests/<dir>/main.rs` is a second binary the
filesystem does not show, and it is caught.

```
error[layout/too-many-test-binaries]: lichen-serve carries 54 external test binaries, and the bound is 1
  found: abi_order, arg_order, attend, ...
  help: gather the areas as modules under tests/<name>/, so the crate carries one binary
checked 29 packages in 2 workspaces against test-layout.toml: 1 violation
```

Exit codes: `0` no violations, `1` violations found, `2` the command could not
run, so nothing was checked.

## The policy file

`test-layout.toml` states the bound, and lives beside the project it governs —
the same arrangement [comments-test](https://github.com/windwhiterain/comments-test)
uses for the comment policy.

```toml
[layout]
max_external_test_binaries = 1
exempt = []
```

When no file is found beside the target or in the current directory, the
built-in policy — one external test binary per crate — is used, and the report
names it.

## The library

`test_layout_check` is the checker as a library: `Checker::new(root, policy)`
reads the census and reports every crate above the bound.

## License

MIT OR Apache-2.0
