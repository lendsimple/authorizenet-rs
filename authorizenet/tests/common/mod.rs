//! Helpers shared by integration tests.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Fixture file names, sorted, whose root element name ends with `suffix`.
pub fn fixtures_with_root_suffix(suffix: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir())
        .expect("tests/data exists")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".xml"))
        .filter(|name| root_name(&fixture(name)).ends_with(suffix))
        .collect();
    names.sort();
    names
}

pub fn root_name(xml: &[u8]) -> String {
    authorizenet::xml::root_name(xml).expect("fixture has a root element")
}

/// Attributes the API sends that the schema does not define, so they are dropped.
const UNDECLARED_ATTRIBUTES: &[&str] = &[
    // ARBGetSubscriptionStatusResponse/@note: "Status with a capital 'S' is obsolete."
    "note",
];

/// Compares two documents as element trees: names, schema attributes, child order,
/// and the trimmed text of leaf elements. Whitespace between elements and namespace
/// declarations are ignored. Returns the first difference.
pub fn compare_trees(expected: &str, actual: &str) -> Result<(), String> {
    let expected = roxmltree::Document::parse(strip_declarations(expected))
        .map_err(|e| format!("expected: {e}"))?;
    let actual = roxmltree::Document::parse(actual).map_err(|e| format!("actual: {e}"))?;
    compare_nodes(expected.root_element(), actual.root_element(), "")
}

/// Drops leading whitespace and XML declarations; one fixture repeats its declaration.
fn strip_declarations(mut xml: &str) -> &str {
    loop {
        xml = xml.trim_start();
        match xml
            .strip_prefix("<?xml")
            .and_then(|rest| rest.split_once("?>"))
        {
            Some((_, rest)) => xml = rest,
            None => return xml,
        }
    }
}

fn compare_nodes(e: roxmltree::Node, a: roxmltree::Node, parent: &str) -> Result<(), String> {
    let path = format!("{parent}/{}", e.tag_name().name());
    if e.tag_name().name() != a.tag_name().name() {
        return Err(format!(
            "{path}: element <{}> where <{}> was expected",
            a.tag_name().name(),
            e.tag_name().name()
        ));
    }
    let attrs = |n: roxmltree::Node| -> Vec<(String, String)> {
        let mut v: Vec<_> = n
            .attributes()
            .filter(|at| at.namespace().is_none() && !UNDECLARED_ATTRIBUTES.contains(&at.name()))
            .map(|at| (at.name().to_owned(), at.value().to_owned()))
            .collect();
        v.sort();
        v
    };
    if attrs(e) != attrs(a) {
        return Err(format!(
            "{path}: attributes {:?} != expected {:?}",
            attrs(a),
            attrs(e)
        ));
    }
    let e_children: Vec<_> = e.children().filter(|n| n.is_element()).collect();
    let a_children: Vec<_> = a.children().filter(|n| n.is_element()).collect();
    if e_children.is_empty() && a_children.is_empty() {
        let text = |n: roxmltree::Node| n.text().unwrap_or_default().trim().to_owned();
        if text(e) != text(a) {
            return Err(format!(
                "{path}: text {:?} != expected {:?}",
                text(a),
                text(e)
            ));
        }
        return Ok(());
    }
    for (i, ec) in e_children.iter().enumerate() {
        match a_children.get(i) {
            Some(ac) => compare_nodes(*ec, *ac, &path)?,
            None => {
                return Err(format!(
                    "{path}: missing <{}> (child {i})",
                    ec.tag_name().name()
                ));
            }
        }
    }
    if let Some(extra) = a_children.get(e_children.len()) {
        return Err(format!("{path}: unexpected <{}>", extra.tag_name().name()));
    }
    Ok(())
}

/// The `<merchantAuthentication>` element of a request fixture, as markup.
pub fn auth_fragment(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let node = doc
        .descendants()
        .find(|n| n.tag_name().name() == "merchantAuthentication")?;
    Some(xml[node.range()].to_owned())
}
