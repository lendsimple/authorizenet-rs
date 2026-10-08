//! The async client against a mock API: the Python client's async tests, ported.
#![cfg(feature = "async")]

#[macro_use]
mod common;

use std::time::Duration;

use authorizenet::schema::{AuthenticateTestRequest, CreateTransactionRequest, MessageType};
use authorizenet::{Client, Credentials, Error};
use common::{MockApi, credentials, fixture};

fn client(api: &MockApi, credentials: Credentials) -> Client {
    Client::builder(credentials)
        .environment(api.environment())
        .build()
        .unwrap()
}

macro_rules! async_cases {
    ($(
        $name:ident: $group:ident.$method:ident, $kind:ident($($arg:tt)*) => $response:literal;
    )*) => {$(
        #[tokio::test]
        async fn $name() {
            let (request, credentials, golden) = case!($kind($($arg)*));
            let api = MockApi::new_async(200, fixture($response)).await;
            let response = client(&api, credentials).$group().$method(&request).await.unwrap();
            assert_eq!(response.messages.result_code, MessageType::Ok);
            api.check_request(golden);
        }
    )*};
}

client_cases!(async_cases);

// test_errors.py

#[tokio::test]
async fn error_response_is_an_api_error() {
    let api = MockApi::new_async(200, fixture("error_response.xml")).await;
    let err = client(&api, credentials())
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .await
        .unwrap_err();
    let api_err = err.as_api().expect("an API error");
    assert_eq!(api_err.code(), "E00035");
    assert_eq!(api_err.text(), "The subscription cannot be found.");
    assert_eq!(
        err.to_string(),
        "[E00035] The subscription cannot be found."
    );
}

#[tokio::test]
async fn subscriptions_get_status_reports_missing_subscription() {
    let api = MockApi::new_async(200, fixture("get_subscription_status_response.xml")).await;
    let request = common::request_fixture("get_subscription_status_request.xml");
    let err = client(
        &api,
        common::fixture_credentials("get_subscription_status_request.xml"),
    )
    .subscriptions()
    .get_status(&request)
    .await
    .unwrap_err();
    assert_eq!(err.as_api().map(|e| e.code()), Some("E00035"));
    api.check_request(Some("get_subscription_status_request.xml"));
}

// Beyond the Python suite

/// The futures can be spawned on a multi-threaded tokio runtime.
#[test]
fn futures_are_send() {
    fn assert_send<T: Send>(_: &T) {}
    let client = Client::new(credentials()).unwrap();
    let request: CreateTransactionRequest =
        common::request_fixture("authorize_credit_card_request.xml");
    assert_send(&client.transactions().create(&request));
    assert_send(&client.execute(&request));
}

#[tokio::test]
async fn clients_can_be_shared_across_tasks() {
    let api = MockApi::new_async(200, fixture("authenticate_test_response.xml")).await;
    let client = client(&api, credentials());
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .misc()
                    .test_authenticate(&AuthenticateTestRequest::default())
                    .await
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap().unwrap();
    }
}

#[tokio::test]
async fn http_error_status() {
    let api = MockApi::new_async(503, b"Service Unavailable".to_vec()).await;
    let result = client(&api, credentials())
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .await;
    assert!(
        matches!(&result, Err(Error::Status { status: 503, body }) if body == "Service Unavailable"),
        "{result:?}"
    );
}

#[tokio::test]
async fn connection_failure_is_a_transport_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let client = Client::builder(credentials())
        .environment(authorizenet::Environment::Custom(format!(
            "http://127.0.0.1:{port}/"
        )))
        .build()
        .unwrap();
    let result = client
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .await;
    assert!(
        matches!(&result, Err(Error::Transport(e)) if !e.is_timeout()),
        "{result:?}"
    );
}

#[tokio::test]
async fn slow_response_times_out() {
    let api = MockApi::delayed_async(
        200,
        fixture("authenticate_test_response.xml"),
        Duration::from_secs(2),
    )
    .await;
    let client = Client::builder(credentials())
        .environment(api.environment())
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let result = client
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .await;
    assert!(
        matches!(&result, Err(Error::Transport(e)) if e.is_timeout()),
        "{result:?}"
    );
}

#[tokio::test]
async fn invalid_request_is_not_sent() {
    let api = MockApi::new_async(200, fixture("authenticate_test_response.xml")).await;
    let request = AuthenticateTestRequest::builder()
        .ref_id("x".repeat(51))
        .build();
    let result = client(&api, credentials())
        .misc()
        .test_authenticate(&request)
        .await;
    assert!(matches!(result, Err(Error::Validation(_))), "{result:?}");
    assert_eq!(api.received(), None);
}
