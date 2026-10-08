//! Request encoding and response decoding, without HTTP.

mod common;

use std::collections::HashSet;

use authorizenet::protocol::{decode, encode, encode_unvalidated};
use authorizenet::schema::{
    AnyResponse, AuthenticateTestRequest, AuthenticateTestResponse,
    CreateCustomerProfileFromTransactionRequest, CreateCustomerProfileResponse,
    CreateTransactionPayload, CreateTransactionRequest, CreateTransactionResponse, ExtendedAmount,
    GetHostedPaymentPageResponse, GetTransactionListForCustomerRequest, GetTransactionListResponse,
    IsAliveRequest, LineItem, MessageType, TransactionRequest, TransactionType,
};
use authorizenet::{ApiError, ApiRequest, ApiResponse, Credentials, Decimal, Error};
use common::fixture;
use pretty_assertions::assert_eq;

fn credentials() -> Credentials {
    Credentials::transaction_key("5KP3u95bQpv", "346HZ32z3fP4hTG2")
}

fn api_error(result: Result<impl std::fmt::Debug, Error>) -> ApiError {
    match result {
        Err(Error::Api(err)) => *err,
        other => panic!("expected an API error, got {other:?}"),
    }
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

// Encoding

#[test]
fn authenticated_request_carries_credentials() {
    let body = encode(&AuthenticateTestRequest::default(), &credentials()).unwrap();
    assert!(body.contains(
        "<merchantAuthentication><name>5KP3u95bQpv</name>\
         <transactionKey>346HZ32z3fP4hTG2</transactionKey></merchantAuthentication>"
    ));
}

#[test]
fn is_alive_is_sent_without_credentials() {
    let body = encode(&IsAliveRequest::default(), &credentials()).unwrap();
    assert!(!body.contains("merchantAuthentication"), "{body}");
    assert!(!body.contains("346HZ32z3fP4hTG2"), "{body}");
}

fn charge(ref_id: &str, line_item: LineItem) -> CreateTransactionRequest {
    let transaction = TransactionRequest::builder()
        .transaction_type(TransactionType::AuthCaptureTransaction)
        .amount(dec("5.00"))
        .line_items(vec![line_item])
        .tax(ExtendedAmount::builder().amount(dec("-1.00")).build())
        .build();
    CreateTransactionRequest::builder()
        .ref_id(ref_id)
        .transaction(CreateTransactionPayload::TransactionRequest(transaction))
        .build()
}

fn vase(quantity: &str) -> LineItem {
    LineItem::builder()
        .item_id("1")
        .name("vase")
        .quantity(dec(quantity))
        .unit_price(dec("45.00"))
        .build()
}

/// The live XSD allows 50 characters (the Python client's older schema had 20).
const LONG_REF_ID: &str = "a reference id that is longer than fifty characters";

#[test]
fn invalid_request_is_rejected_before_sending() {
    let request = charge(LONG_REF_ID, vase("1.23456"));
    let Err(Error::Validation(err)) = encode(&request, &credentials()) else {
        panic!("expected a validation error");
    };
    let violations: Vec<String> = err.violations().iter().map(ToString::to_string).collect();
    assert_eq!(
        violations,
        [
            "refId: must be at most 50 characters, not 51",
            "transactionRequest.lineItems[0].quantity: \
             must have at most 4 digits after the decimal point, not 5",
            "transactionRequest.tax.amount: must be at least 0.00, not -1.00",
        ]
    );
}

#[test]
fn validation_can_be_skipped() {
    let request = charge(LONG_REF_ID, vase("1"));
    assert!(encode_unvalidated(&request, &credentials()).is_ok());
}

// Decoding

#[test]
fn success_response_is_returned() {
    let response: AuthenticateTestResponse =
        decode(200, &fixture("authenticate_test_response.xml")).unwrap();
    assert_eq!(response.messages().result_code, MessageType::Ok);
}

#[test]
fn error_response_is_an_api_error() {
    let err = api_error(decode::<AuthenticateTestResponse>(
        200,
        &fixture("error_response.xml"),
    ));
    assert_eq!(err.code(), "E00035");
    assert_eq!(err.text(), "The subscription cannot be found.");
    assert_eq!(err.messages().result_code, MessageType::Error);
    assert!(matches!(
        err.any_response(),
        Some(AnyResponse::ErrorResponse(_))
    ));
}

#[test]
fn api_error_displays_first_code_and_text() {
    let result = decode::<AuthenticateTestResponse>(200, &fixture("error_response.xml"));
    assert_eq!(
        result.unwrap_err().to_string(),
        "[E00035] The subscription cannot be found."
    );
}

#[test]
fn declined_transaction_keeps_its_transaction_response() {
    let err = api_error(decode::<CreateTransactionResponse>(
        200,
        &fixture("create_transaction_declined_response.xml"),
    ));
    assert_eq!(err.code(), "E00027");
    let response = err
        .response::<CreateTransactionResponse>()
        .expect("typed response is kept");
    let error = &response.transaction_response.errors[0];
    assert_eq!(error.error_code.as_deref(), Some("2"));
    assert_eq!(
        error.error_text.as_deref(),
        Some("This transaction has been declined.")
    );
    assert!(err.response::<AuthenticateTestResponse>().is_none());
}

#[test]
fn error_without_required_fields_falls_back_to_messages() {
    // getHostedPaymentPageResponse requires <token>, which an error leaves out.
    let body = br#"<?xml version="1.0" encoding="utf-8"?>
        <getHostedPaymentPageResponse xmlns="AnetApi/xml/v1/schema/AnetApiSchema.xsd">
            <messages><resultCode>Error</resultCode>
                <message><code>E00007</code><text>User authentication failed due to invalid authentication values.</text></message>
            </messages>
        </getHostedPaymentPageResponse>"#;
    let err = api_error(decode::<GetHostedPaymentPageResponse>(200, body));
    assert_eq!(err.code(), "E00007");
    assert_eq!(err.any_response(), None);
}

#[test]
fn success_without_required_fields_is_an_xml_error() {
    let body = br#"<getHostedPaymentPageResponse>
            <messages><resultCode>Ok</resultCode>
                <message><code>I00001</code><text>Successful.</text></message>
            </messages>
        </getHostedPaymentPageResponse>"#;
    let result = decode::<GetHostedPaymentPageResponse>(200, body);
    assert!(matches!(result, Err(Error::Xml(_))), "{result:?}");
}

#[test]
fn unexpected_response_type_is_an_xml_error() {
    let result =
        decode::<CreateTransactionResponse>(200, &fixture("authenticate_test_response.xml"));
    assert!(matches!(result, Err(Error::Xml(_))), "{result:?}");
}

#[test]
fn http_error_status_keeps_the_body() {
    let result = decode::<AuthenticateTestResponse>(503, b"Service Unavailable");
    assert!(
        matches!(&result, Err(Error::Status { status: 503, body }) if body == "Service Unavailable"),
        "{result:?}"
    );
}

#[test]
fn response_with_byte_order_mark_is_read() {
    let mut body = b"\xEF\xBB\xBF".to_vec();
    body.extend(fixture("authenticate_test_response.xml"));
    assert!(decode::<AuthenticateTestResponse>(200, &body).is_ok());
}

// The operation table

#[test]
fn irregular_pairings() {
    fn response_of<R: ApiRequest>() -> &'static str {
        <R::Response as authorizenet::xml::XmlRoot>::ROOT
    }
    assert_eq!(
        response_of::<CreateCustomerProfileFromTransactionRequest>(),
        <CreateCustomerProfileResponse as authorizenet::xml::XmlRoot>::ROOT
    );
    assert_eq!(
        response_of::<GetTransactionListForCustomerRequest>(),
        <GetTransactionListResponse as authorizenet::xml::XmlRoot>::ROOT
    );
}

/// Every request type is in the operation table, under a unique name.
macro_rules! operation_names {
    ($($ty:path),* $(,)?) => {
        fn operation_names() -> Vec<&'static str> {
            vec![$(<$ty as ApiRequest>::OPERATION),*]
        }
    };
}

authorizenet::__for_each_request!(operation_names);

#[test]
fn every_request_has_a_unique_operation() {
    let names = operation_names();
    let unique: HashSet<_> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "duplicate operation names in {names:?}"
    );
    assert!(names.contains(&"transactions.create"));
    assert!(names.contains(&"misc.is_alive"));
}
