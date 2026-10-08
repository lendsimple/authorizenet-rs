//! The blocking client against a mock API: the Python client's sync tests, ported.
#![cfg(feature = "ureq")]

#[macro_use]
mod common;

use std::time::Duration;

use authorizenet::blocking::Client;
use authorizenet::schema::{AuthenticateTestRequest, MessageType};
use authorizenet::{Credentials, Error};
use common::{MockApi, credentials, fixture};

fn client(api: &MockApi, credentials: Credentials) -> Client {
    Client::builder(credentials)
        .environment(api.environment())
        .build()
}

macro_rules! blocking_cases {
    ($(
        $name:ident: $group:ident.$method:ident, $kind:ident($($arg:tt)*) => $response:literal;
    )*) => {$(
        #[test]
        fn $name() {
            let (request, credentials, golden) = case!($kind($($arg)*));
            let api = MockApi::new(200, fixture($response));
            let response = client(&api, credentials).$group().$method(&request).unwrap();
            assert_eq!(response.messages.result_code, MessageType::Ok);
            api.check_request(golden);
        }
    )*};
}

client_cases!(blocking_cases);

// test_errors.py

#[test]
fn error_response_is_an_api_error() {
    let api = MockApi::new(200, fixture("error_response.xml"));
    let err = client(&api, credentials())
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .unwrap_err();
    let api_err = err.as_api().expect("an API error");
    assert_eq!(api_err.code(), "E00035");
    assert_eq!(api_err.text(), "The subscription cannot be found.");
    assert_eq!(api_err.messages().result_code, MessageType::Error);
    assert_eq!(
        err.to_string(),
        "[E00035] The subscription cannot be found."
    );
}

#[test]
fn subscriptions_get_status_reports_missing_subscription() {
    let api = MockApi::new(200, fixture("get_subscription_status_response.xml"));
    let request = common::request_fixture("get_subscription_status_request.xml");
    let err = client(
        &api,
        common::fixture_credentials("get_subscription_status_request.xml"),
    )
    .subscriptions()
    .get_status(&request)
    .unwrap_err();
    assert_eq!(err.as_api().map(|e| e.code()), Some("E00035"));
    api.check_request(Some("get_subscription_status_request.xml"));
}

// Beyond the Python suite

#[test]
fn http_error_status() {
    let api = MockApi::new(500, b"Internal Server Error".to_vec());
    let result = client(&api, credentials())
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default());
    assert!(
        matches!(&result, Err(Error::Status { status: 500, body }) if body == "Internal Server Error"),
        "{result:?}"
    );
}

#[test]
fn connection_failure_is_a_transport_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = Client::builder(credentials())
        .environment(authorizenet::Environment::Custom(format!(
            "http://127.0.0.1:{port}/"
        )))
        .build();
    let result = client
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default());
    assert!(
        matches!(&result, Err(Error::Transport(e)) if !e.is_timeout()),
        "{result:?}"
    );
}

#[test]
fn slow_response_times_out() {
    let api = MockApi::delayed(
        200,
        fixture("authenticate_test_response.xml"),
        Duration::from_secs(2),
    );
    let client = Client::builder(credentials())
        .environment(api.environment())
        .timeout(Duration::from_millis(200))
        .build();
    let result = client
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default());
    assert!(
        matches!(&result, Err(Error::Transport(e)) if e.is_timeout()),
        "{result:?}"
    );
}

fn too_long_ref_id() -> AuthenticateTestRequest {
    AuthenticateTestRequest::builder()
        .ref_id("x".repeat(51))
        .build()
}

#[test]
fn invalid_request_is_not_sent() {
    let api = MockApi::new(200, fixture("authenticate_test_response.xml"));
    let result = client(&api, credentials())
        .misc()
        .test_authenticate(&too_long_ref_id());
    assert!(matches!(result, Err(Error::Validation(_))), "{result:?}");
    assert_eq!(api.received(), None);
}

#[test]
fn validation_can_be_turned_off() {
    let api = MockApi::new(200, fixture("authenticate_test_response.xml"));
    let client = Client::builder(credentials())
        .environment(api.environment())
        .validate_requests(false)
        .build();
    client.misc().test_authenticate(&too_long_ref_id()).unwrap();
    assert!(api.received().is_some());
}
