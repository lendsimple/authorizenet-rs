//! Smoke tests against the real Authorize.Net sandbox. They are ignored by default;
//! run them with sandbox credentials:
//!
//! ```sh
//! ANET_LOGIN_ID=... ANET_TRANSACTION_KEY=... cargo test --all-features --test sandbox -- --ignored
//! ```
#![cfg(all(feature = "async", feature = "blocking"))]

use std::time::{SystemTime, UNIX_EPOCH};

use authorizenet::schema::{
    AuthenticateTestRequest, CreateCustomerProfileRequest, CreateTransactionRequest, CreditCard,
    CustomerPaymentProfile, CustomerProfile, CustomerProfileBase, DeleteCustomerProfileRequest,
    GetCustomerProfileRequest, IsAliveRequest, TransactionOutcome, TransactionRequest,
    ValidationMode,
};
use authorizenet::{Client, Credentials, Decimal};

fn credentials() -> Credentials {
    let var = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| {
            panic!(
                "the sandbox tests need ANET_LOGIN_ID and ANET_TRANSACTION_KEY ({name} is not set)"
            )
        })
    };
    Credentials::transaction_key(var("ANET_LOGIN_ID"), var("ANET_TRANSACTION_KEY"))
}

fn client() -> Client {
    Client::new(credentials()).unwrap()
}

/// Differs between runs, so the sandbox does not reject a charge as a duplicate.
fn unique() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn test_card() -> CreditCard {
    CreditCard::new("4111111111111111", "2035-12").with_code("123")
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn api_is_alive() {
    client()
        .misc()
        .is_alive(&IsAliveRequest::default())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn credentials_authenticate() {
    let response = client()
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .await
        .unwrap();
    assert!(response.messages.is_ok());
}

#[test]
#[ignore = "needs sandbox credentials"]
fn blocking_client_authenticates() {
    let client = authorizenet::blocking::Client::new(credentials()).unwrap();
    let response = client
        .misc()
        .test_authenticate(&AuthenticateTestRequest::default())
        .unwrap();
    assert!(response.messages.is_ok());
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn charge_and_void() {
    let client = client();
    let amount = Decimal::new(100 + (unique() % 9900) as i64, 2);
    let charge =
        CreateTransactionRequest::from(TransactionRequest::auth_capture(amount, test_card()));
    let charged = client.transactions().create(&charge).await.unwrap();
    let transaction = charged.transaction_response;
    assert_eq!(transaction.outcome(), Some(TransactionOutcome::Approved));
    let id = transaction.trans_id.expect("an approved charge has an id");

    let void = CreateTransactionRequest::from(TransactionRequest::void(&id));
    let voided = client.transactions().create(&void).await.unwrap();
    assert_eq!(
        voided.transaction_response.outcome(),
        Some(TransactionOutcome::Approved)
    );
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn customer_profile_lifecycle() {
    let client = client();
    let profile = CustomerProfile::builder()
        .customer_profile_base(
            CustomerProfileBase::builder()
                .merchant_customer_id(format!("rs{}", unique() % 10_000_000_000))
                .email("sandbox-test@example.com")
                .build(),
        )
        .payment_profiles(vec![
            CustomerPaymentProfile::builder()
                .payment(test_card().into())
                .build(),
        ])
        .build();
    let created = client
        .customer_profiles()
        .create(
            &CreateCustomerProfileRequest::builder()
                .profile(profile)
                .validation_mode(ValidationMode::None)
                .build(),
        )
        .await
        .unwrap();
    let id = created
        .customer_profile_id
        .expect("a new profile has an id");

    let fetched = client
        .customer_profiles()
        .get(
            &GetCustomerProfileRequest::builder()
                .customer_profile_id(&id)
                .build(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.profile.map(|p| p.payment_profiles.len()), Some(1));

    client
        .customer_profiles()
        .delete(
            &DeleteCustomerProfileRequest::builder()
                .customer_profile_id(&id)
                .build(),
        )
        .await
        .unwrap();
}
