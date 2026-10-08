//! Every request and response fixture from the Python client's test suite, against
//! the generated schema types.
//!
//! Each fixture is parsed strictly (no unknown elements), written back out, and the
//! output compared to the fixture as an element tree. This checks element names,
//! nesting, sequence order and values in both directions.

mod common;

use std::path::Path;
use std::process::Command;

use authorizenet::schema::MerchantAuthentication;
use authorizenet::xml::{self, XmlError, XmlRoot};
use common::{auth_fragment, compare_trees, fixture, fixtures_with_root_suffix, root_name};

/// `fn reserialize_request(root, input) -> Result<String, XmlError>` over every
/// generated request type.
macro_rules! request_dispatch {
    ($($ty:path),* $(,)?) => {
        fn reserialize_request(root: &str, input: &[u8]) -> Result<String, XmlError> {
            $(
                if root == <$ty as XmlRoot>::ROOT {
                    let value: $ty = xml::from_slice_strict(input)?;
                    if !<$ty as XmlRoot>::AUTHENTICATED {
                        return xml::to_string(&value);
                    }
                    let text = std::str::from_utf8(input).expect("UTF-8 fixture");
                    let auth = auth_fragment(text).expect("request fixture has merchantAuthentication");
                    let auth: MerchantAuthentication = xml::from_slice_as(auth.as_bytes())?;
                    return xml::to_string_with_auth(&value, &auth);
                }
            )*
            panic!("no generated request type has root <{root}>")
        }
    };
}

/// `fn reserialize_response(root, input) -> Result<String, XmlError>`, which also
/// checks that the output parses back to an equal value.
macro_rules! response_dispatch {
    ($($ty:path),* $(,)?) => {
        fn reserialize_response(root: &str, input: &[u8]) -> Result<String, XmlError> {
            $(
                if root == <$ty as XmlRoot>::ROOT {
                    let value: $ty = xml::from_slice_strict(input)?;
                    let out = xml::to_string(&value)?;
                    let again: $ty = xml::from_slice_strict(out.as_bytes())?;
                    assert!(again == value, "<{root}> changed after a round trip");
                    return Ok(out);
                }
            )*
            panic!("no generated response type has root <{root}>")
        }
    };
}

authorizenet::__for_each_request!(request_dispatch);
authorizenet::__for_each_response!(response_dispatch);

type Reserialize = fn(&str, &[u8]) -> Result<String, XmlError>;

/// Fixtures for messages the live XSD no longer defines (see docs/schema-drift.md).
const REMOVED_FROM_XSD: &[&str] = &[
    // mobileDeviceLoginResponse was replaced by the PIN and verify login flows.
    "mobile_device_login_response.xml",
];

/// Fixtures whose sample data breaks an XSD facet, so they are not schema-validated.
const INVALID_SAMPLE_DATA: &[&str] = &[
    // <customerProfileId>YourProfileID</customerProfileId> must match [0-9]+.
    "get_hosted_profile_page_request.xml",
];

/// Runs every fixture through `reserialize`, returning `(name, output)` and failing
/// with the full list of problems.
fn check_all(suffix: &str, reserialize: Reserialize) -> Vec<(String, String)> {
    let names: Vec<String> = fixtures_with_root_suffix(suffix)
        .into_iter()
        .filter(|name| !REMOVED_FROM_XSD.contains(&name.as_str()))
        .collect();
    assert!(!names.is_empty(), "no *{suffix} fixtures found");
    let mut outputs = Vec::new();
    let mut failures = Vec::new();
    for name in names {
        let input = fixture(&name);
        let expected = String::from_utf8(input.clone()).expect("UTF-8 fixture");
        let result = reserialize(&root_name(&input), &input)
            .map_err(|e| e.to_string())
            .and_then(|out| compare_trees(&expected, &out).map(|()| out));
        match result {
            Ok(out) => outputs.push((name, out)),
            Err(err) => failures.push(format!("{name}: {err}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixtures failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    outputs
}

#[test]
fn request_fixtures_round_trip() {
    check_all("Request", reserialize_request);
}

#[test]
fn response_fixtures_round_trip() {
    check_all("Response", reserialize_response);
}

/// Validates the serialized requests against the vendored XSD with `xmllint`, when
/// it is installed.
#[test]
fn serialized_requests_conform_to_xsd() {
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("skipping: xmllint is not installed");
        return;
    }
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/AnetApiSchema.xsd");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("conformance");
    std::fs::create_dir_all(&dir).unwrap();
    let mut files = Vec::new();
    for (name, out) in check_all("Request", reserialize_request) {
        if INVALID_SAMPLE_DATA.contains(&name.as_str()) {
            continue;
        }
        let path = dir.join(name);
        std::fs::write(&path, out).unwrap();
        files.push(path);
    }
    let output = Command::new("xmllint")
        .arg("--noout")
        .arg("--nowarning")
        .arg("--schema")
        .arg(&schema)
        .args(&files)
        .output()
        .expect("xmllint runs");
    assert!(
        output.status.success(),
        "xmllint rejected serialized requests:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
