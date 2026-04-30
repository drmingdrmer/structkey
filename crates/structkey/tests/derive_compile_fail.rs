//! Compile-fail snapshots for `#[derive(Codec)]`.
//!
//! Locks in the rejection error message for input shapes the macro does
//! not support: enums, tuple structs, unit structs, unions, raw on a
//! non-`String` field, and unknown `#[codec]` sub-options.
//!
//! When the rustc error format changes (e.g. on a toolchain bump),
//! regenerate the `tests/ui/*.stderr` snapshots with:
//!
//! ```text
//! TRYBUILD=overwrite cargo test --test derive_compile_fail
//! ```

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
