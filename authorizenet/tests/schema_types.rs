//! Targeted checks of generated schema types.

mod common;

use authorizenet::schema::{
    AuthenticateTestRequest, AuthenticateTestResponse, CreditCardTrack, CustomerAddress,
    ErrorResponse, GetCustomerProfileIdsResponse, IsAliveRequest, MerchantAuthentication,
    MerchantCredential, MessageType, NameAndAddress, Payment, PaymentInstrument,
};
use authorizenet::xml::{self, XmlRoot, XmlWrite, XmlWriter};
use common::fixture;
use pretty_assertions::assert_eq;

fn credentials() -> MerchantAuthentication {
    MerchantAuthentication::builder()
        .name("5KP3u95bQpv")
        .credential(MerchantCredential::TransactionKey(
            "346HZ32z3fP4hTG2".into(),
        ))
        .build()
}

fn element(value: &impl XmlWrite, tag: &str) -> String {
    let mut w = XmlWriter::new(Vec::new());
    value.write_element(&mut w, tag).unwrap();
    String::from_utf8(w.into_inner()).unwrap()
}

#[test]
fn error_response_fields() {
    let response: ErrorResponse = xml::from_slice_strict(&fixture("error_response.xml")).unwrap();
    assert_eq!(response.messages.result_code, MessageType::Error);
    assert_eq!(response.messages.message[0].code, "E00035");
    assert_eq!(
        response.messages.message[0].text,
        "The subscription cannot be found."
    );
}

#[test]
fn any_response_parses_as_error_response_fields() {
    let response: ErrorResponse =
        xml::from_slice_as(&fixture("authenticate_test_response.xml")).unwrap();
    assert_eq!(response.messages.result_code, MessageType::Ok);
}

#[test]
fn authenticated_request_writes_merchant_authentication_first() {
    let request = AuthenticateTestRequest::builder().ref_id("123456").build();
    assert_eq!(
        xml::to_string_with_auth(&request, &credentials()).unwrap(),
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
fn only_api_requests_are_authenticated() {
    const { assert!(AuthenticateTestRequest::AUTHENTICATED) };
    const { assert!(!IsAliveRequest::AUTHENTICATED) };
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
    assert_eq!(
        element(&address, "billTo"),
        "<billTo><firstName>Ellen</firstName><zip>44628</zip>\
         <phoneNumber>555-1234</phoneNumber></billTo>"
    );
}

#[test]
fn bare_choice_type_is_an_element() {
    let track = CreditCardTrack::Track2(";4111111111111111=2512?".into());
    let out = element(&track, "trackData");
    assert_eq!(
        out,
        "<trackData><track2>;4111111111111111=2512?</track2></trackData>"
    );
}

#[test]
fn choice_inside_sequence_comes_before_later_fields() {
    let payment = Payment::builder()
        .instrument(PaymentInstrument::TrackData(CreditCardTrack::Track1(
            "%B4111".into(),
        )))
        .data_source("swipe")
        .build();
    assert_eq!(
        element(&payment, "payment"),
        "<payment><trackData><track1>%B4111</track1></trackData>\
         <dataSource>swipe</dataSource></payment>"
    );
}

#[test]
fn required_list_wrapper_is_written_when_empty() {
    let response: GetCustomerProfileIdsResponse = xml::from_str(
        "<getCustomerProfileIdsResponse><messages><resultCode>Ok</resultCode>\
         <message><code>I00001</code><text>Successful.</text></message></messages>\
         </getCustomerProfileIdsResponse>",
    )
    .unwrap();
    assert!(response.ids.is_empty());
    assert!(xml::to_string(&response).unwrap().contains("<ids></ids>"));
}

#[test]
fn debug_redacts_credentials() {
    let debug = format!("{:?}", credentials());
    assert!(!debug.contains("346HZ32z3fP4hTG2"), "{debug}");
    assert!(debug.contains("TransactionKey([REDACTED])"), "{debug}");
    assert!(debug.contains("5KP3u95bQpv"), "{debug}");
}

#[test]
fn debug_redacts_card_track_data() {
    let debug = format!("{:?}", CreditCardTrack::Track1("%B4111111111111111".into()));
    assert!(!debug.contains("4111"), "{debug}");
}
