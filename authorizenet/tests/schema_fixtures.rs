//! The hand-written schema types against fixtures from the Python client's test suite.

use std::path::PathBuf;

use authorizenet::schema::{
    AuthenticateTestRequest, AuthenticateTestResponse, CustomerAddress, ErrorResponse,
    MerchantAuthentication, MerchantCredential, MessageType, NameAndAddress,
};
use authorizenet::xml::{self, XmlRoot};
use pretty_assertions::assert_eq;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Parses strictly, serializes, and parses again; both parses must agree.
fn round_trip<T: XmlRoot + PartialEq + std::fmt::Debug>(name: &str) -> T {
    let parsed: T = xml::from_slice_strict(&fixture(name)).unwrap();
    let written = xml::to_string(&parsed).unwrap();
    let reparsed: T = xml::from_slice_strict(written.as_bytes()).unwrap();
    assert_eq!(reparsed, parsed);
    parsed
}

fn credentials() -> MerchantAuthentication {
    MerchantAuthentication::builder()
        .name("5KP3u95bQpv")
        .credential(MerchantCredential::TransactionKey(
            "346HZ32z3fP4hTG2".into(),
        ))
        .build()
}

#[test]
fn error_response_round_trips() {
    let response: ErrorResponse = round_trip("error_response.xml");
    let messages = &response.base.messages;
    assert_eq!(messages.result_code, MessageType::Error);
    assert_eq!(messages.message[0].code, "E00035");
    assert_eq!(
        messages.message[0].text,
        "The subscription cannot be found."
    );
}

#[test]
fn authenticate_test_response_round_trips() {
    let response: AuthenticateTestResponse = round_trip("authenticate_test_response.xml");
    let messages = &response.base.messages;
    assert_eq!(messages.result_code, MessageType::Ok);
    assert_eq!(messages.message[0].code, "I00001");
    assert_eq!(messages.message[0].text, "Successful.");
}

#[test]
fn error_response_is_not_an_authenticate_test_response() {
    let err =
        xml::from_slice::<AuthenticateTestResponse>(&fixture("error_response.xml")).unwrap_err();
    assert!(matches!(
        err,
        xml::XmlError::UnexpectedRoot { ref found, .. } if found == "ErrorResponse"
    ));
}

#[test]
fn root_name_identifies_error_response() {
    assert_eq!(
        xml::root_name(&fixture("error_response.xml")).unwrap(),
        "ErrorResponse"
    );
}

#[test]
fn request_writes_merchant_authentication_first() {
    let request = AuthenticateTestRequest::builder().ref_id("123456").build();
    let out = xml::to_string_with_auth(&request, &credentials()).unwrap();
    assert_eq!(
        out,
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<authenticateTestRequest xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd">"#,
            "<merchantAuthentication><name>5KP3u95bQpv</name>",
            "<transactionKey>346HZ32z3fP4hTG2</transactionKey></merchantAuthentication>",
            "<refId>123456</refId>",
            "</authenticateTestRequest>",
        )
    );
}

#[test]
fn reading_request_skips_merchant_authentication() {
    let request = AuthenticateTestRequest::builder().ref_id("1").build();
    let out = xml::to_string_with_auth(&request, &credentials()).unwrap();
    let parsed: AuthenticateTestRequest = xml::from_slice_strict(out.as_bytes()).unwrap();
    assert_eq!(parsed, request);
}

#[test]
fn request_root_is_authenticated() {
    const { assert!(AuthenticateTestRequest::AUTHENTICATED) };
    const { assert!(!AuthenticateTestResponse::AUTHENTICATED) };
}

#[test]
fn extension_writes_base_fields_first() {
    let address = CustomerAddress::builder()
        .name_and_address(
            NameAndAddress::builder()
                .first_name("Ellen")
                .zip("44628")
                .build(),
        )
        .phone_number("555-1234")
        .build();
    let mut out = Vec::new();
    let mut w = test_writer(&mut out);
    xml::XmlWrite::write_element(&address, &mut w, "billTo").unwrap();
    drop(w);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "<billTo><firstName>Ellen</firstName><zip>44628</zip>\
         <phoneNumber>555-1234</phoneNumber></billTo>"
    );
}

#[test]
fn debug_redacts_transaction_key() {
    let debug = format!("{:?}", credentials());
    assert!(!debug.contains("346HZ32z3fP4hTG2"), "{debug}");
    assert!(debug.contains("TransactionKey([REDACTED])"), "{debug}");
    assert!(debug.contains("5KP3u95bQpv"), "{debug}");
}

fn test_writer(out: &mut Vec<u8>) -> xml::XmlWriter<&mut Vec<u8>> {
    xml::XmlWriter::new(out)
}
