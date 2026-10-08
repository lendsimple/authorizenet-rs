//! Compile-fail checks: misuse of the derives must give a clear, well-placed error.
//!
//! The expected errors follow one rustc's wording, so this only runs in the
//! repository, where `.cargo/config.toml` sets `AUTHORIZENET_UI_TESTS`. From the
//! published crate (crater, distributions) it skips.

#[test]
fn derive_errors() {
    if std::env::var_os("AUTHORIZENET_UI_TESTS").is_none() {
        eprintln!("skipping: compile-error snapshots run only in the repository");
        return;
    }
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
