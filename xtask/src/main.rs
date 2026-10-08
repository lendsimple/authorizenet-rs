//! Development tasks: `cargo xtask <task>`.

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
    codegen [--check]   generate schema types from schema/AnetApiSchema.xsd";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("codegen") => {
            eprintln!("codegen is not implemented yet (Stage 2 of implementation.md)");
            ExitCode::FAILURE
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
