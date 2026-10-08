//! Compile-fail checks: misuse of the derives must give a clear, well-placed error.

#[test]
fn derive_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
