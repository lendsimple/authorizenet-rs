//! Development tasks: `cargo xtask <task>`.

mod drift;
mod emit;
mod model;
mod xsd;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <task>

tasks:
    codegen [--check]   generate schema types from schema/AnetApiSchema.xsd
                        (--check: fail if the committed files are out of date)
    drift --python DIR  write docs/schema-drift.md, comparing the XSD with the
                        python-authorizenet checkout in DIR";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => codegen(false),
        ["codegen", "--check"] => codegen(true),
        ["drift", "--python", dir] => drift(Path::new(dir)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the workspace")
        .to_path_buf()
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()).into())
}

fn load_overrides() -> Result<model::Overrides> {
    let path = workspace_root().join("schema/overrides.toml");
    Ok(toml::from_str(&read(&path)?).map_err(|e| format!("schema/overrides.toml: {e}"))?)
}

/// Builds the model from the vendored schema and overrides.
pub fn load_model() -> Result<model::Model> {
    let schema = xsd::parse(&read(&workspace_root().join("schema/AnetApiSchema.xsd"))?)?;
    Ok(model::build(&schema, &load_overrides()?)?)
}

fn drift(python_dir: &Path) -> Result<()> {
    let root = workspace_root();
    let schema = xsd::parse(&read(&root.join("schema/AnetApiSchema.xsd"))?)?;
    let schema_py = read(&python_dir.join("authorizenet/schema.py"))?;
    let path = root.join("docs/schema-drift.md");
    std::fs::create_dir_all(path.parent().expect("has parent"))?;
    std::fs::write(&path, drift::report(&schema, &schema_py))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn codegen(check: bool) -> Result<()> {
    let model = load_model()?;
    let sensitive = load_overrides()?.sensitive;
    let out_dir = workspace_root().join("authorizenet/src/schema");
    let mut stale = Vec::new();
    for (name, contents) in emit::render(&model, &sensitive) {
        let path = out_dir.join(name);
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == contents {
            continue;
        }
        if check {
            stale.push(path.display().to_string());
        } else {
            std::fs::create_dir_all(&out_dir)?;
            std::fs::write(&path, contents)?;
            println!("wrote {}", path.display());
        }
    }
    if !stale.is_empty() {
        return Err(format!(
            "generated code is out of date; run `cargo xtask codegen`:\n  {}",
            stale.join("\n  ")
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Snapshots of how the trickiest XSD types map to Rust. Review changes with
    //! `cargo insta review` (or rerun with `INSTA_UPDATE=always`).

    use super::load_model;
    use crate::model::Model;

    fn structure(model: &Model, name: &str) -> String {
        if let Some(def) = model.structs.iter().find(|s| s.name == name) {
            return format!("{def:#?}");
        }
        if let Some(def) = model.choices.iter().find(|c| c.name == name) {
            return format!("{def:#?}");
        }
        panic!("no generated type {name}");
    }

    #[test]
    fn transaction_response() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(structure(&model, "TransactionResponse"));
    }

    #[test]
    fn account_updater_details_keep_interleaved_order() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(format!(
            "{}\n{}",
            structure(&model, "ListOfAuDetails"),
            structure(&model, "AuDetail")
        ));
    }

    #[test]
    fn merchant_authentication() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(format!(
            "{}\n{}",
            structure(&model, "MerchantAuthentication"),
            structure(&model, "MerchantCredential")
        ));
    }

    #[test]
    fn payment() {
        let model = load_model().unwrap();
        insta::assert_snapshot!(format!(
            "{}\n{}",
            structure(&model, "Payment"),
            structure(&model, "PaymentInstrument")
        ));
    }
}
