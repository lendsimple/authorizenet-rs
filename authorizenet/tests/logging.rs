//! What the clients log. A separate test binary, because these tests install a global
//! `tracing` subscriber.
#![cfg(any(feature = "async", feature = "blocking"))]

#[macro_use]
mod common;

use std::sync::OnceLock;

use common::{LogCapture, MockApi, credentials, fixture};

/// Card number, CVV and transaction key in the request fixture.
const SECRETS: [&str; 3] = ["5424000000000015", ">999<", "346HZ32z3fP4hTG2"];

fn logs() -> &'static LogCapture {
    static LOGS: OnceLock<LogCapture> = OnceLock::new();
    LOGS.get_or_init(|| {
        let logs = LogCapture::default();
        tracing::subscriber::set_global_default(logs.subscriber()).unwrap();
        logs
    })
}

fn assert_redacted(logs: &str) {
    assert!(logs.contains("operation=\"transactions.create\""), "{logs}");
    assert!(logs.contains("[REDACTED]"), "{logs}");
    for secret in SECRETS {
        assert!(!logs.contains(secret), "{secret} leaked into logs:\n{logs}");
    }
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_client_logs_redact_secrets() {
    let logs = logs();
    let api = MockApi::new(200, fixture("authorize_credit_card_response.xml"));
    let client = authorizenet::blocking::Client::builder(credentials())
        .environment(api.environment())
        .build()
        .unwrap();
    let request = common::request_fixture("authorize_credit_card_request.xml");
    client.transactions().create(&request).unwrap();
    assert_redacted(&logs.contents());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_client_logs_redact_secrets() {
    let logs = logs();
    let api = MockApi::new_async(200, fixture("authorize_credit_card_response.xml")).await;
    let client = authorizenet::Client::builder(credentials())
        .environment(api.environment())
        .build()
        .unwrap();
    let request = common::request_fixture("authorize_credit_card_request.xml");
    client.transactions().create(&request).await.unwrap();
    assert_redacted(&logs.contents());
}
