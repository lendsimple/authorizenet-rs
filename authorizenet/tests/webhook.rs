//! Webhook notifications, read from `tests/data/webhooks`.
//!
//! The fixtures for authcapture, fraud held and approved, customer, payment profile,
//! subscription and boarding events are Authorize.Net's documented samples; the other
//! payment events follow the documented payment payload.

#![cfg(feature = "webhooks")]

mod common;

use authorizenet::Decimal;
use authorizenet::schema::{ArbSubscriptionStatus, CustomerType, TransactionOutcome};
use authorizenet::webhook::{EventType, Notification, Payload, TransactionPayload, parse};
use common::fixture;
use pretty_assertions::assert_eq;
use time::macros::datetime;

fn notification(event: &str) -> Notification {
    let body = fixture(&format!("webhooks/{event}.json"));
    parse(&body).unwrap_or_else(|e| panic!("parsing {event}: {e}"))
}

fn transaction(notification: &Notification) -> &TransactionPayload {
    match &notification.payload {
        Payload::Transaction(transaction) => transaction,
        other => panic!("expected a transaction, got {other:?}"),
    }
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

#[test]
fn payment_events_carry_their_transaction() {
    let cases = [
        (
            "payment.authorization.created",
            EventType::AuthorizationCreated,
            "60020981677",
            "19.99",
        ),
        (
            "payment.authcapture.created",
            EventType::AuthCaptureCreated,
            "60020981676",
            "45.00",
        ),
        (
            "payment.capture.created",
            EventType::CaptureCreated,
            "60020981678",
            "19.99",
        ),
        (
            "payment.priorAuthCapture.created",
            EventType::PriorAuthCaptureCreated,
            "60020981677",
            "19.99",
        ),
        (
            "payment.refund.created",
            EventType::RefundCreated,
            "60020981679",
            "5.10",
        ),
        (
            "payment.void.created",
            EventType::VoidCreated,
            "60020981680",
            "0.29",
        ),
    ];
    for (event, event_type, id, amount) in cases {
        let notification = notification(event);
        assert_eq!(notification.event_type, event_type, "{event}");
        let transaction = transaction(&notification);
        assert_eq!(transaction.id, id, "{event}");
        assert_eq!(transaction.auth_amount, Some(dec(amount)), "{event}");
        assert_eq!(
            transaction.response_code,
            Some(TransactionOutcome::Approved),
            "{event}"
        );
    }
}

#[test]
fn authcapture_reads_every_field() {
    let notification = notification("payment.authcapture.created");
    assert_eq!(
        notification.notification_id,
        "d0e8e7fe-c3e7-4add-a480-27bc5ce28a18"
    );
    assert_eq!(
        notification.webhook_id,
        "63d6fea2-aa13-4b1d-a204-f5fbc15942b7"
    );
    assert_eq!(
        notification.event_date,
        datetime!(2017-03-29 20:48:02.0080095 UTC)
    );
    let transaction = transaction(&notification);
    assert_eq!(transaction.auth_code.as_deref(), Some("LZ6I19"));
    assert_eq!(transaction.avs_response.as_deref(), Some("Y"));
    assert_eq!(
        transaction.merchant_reference_id.as_deref(),
        Some("19102146534003137356")
    );
    assert_eq!(transaction.invoice_number, None);
    assert!(transaction.fraud_list.is_empty());
}

#[test]
fn amounts_are_exact() {
    let refund = notification("payment.refund.created");
    assert_eq!(transaction(&refund).auth_amount.unwrap().to_string(), "5.1");
    let void = notification("payment.void.created");
    assert_eq!(transaction(&void).auth_amount.unwrap().to_string(), "0.29");
}

#[test]
fn refunds_and_voids_have_no_auth_code() {
    for event in ["payment.refund.created", "payment.void.created"] {
        let notification = notification(event);
        let transaction = transaction(&notification);
        assert_eq!(transaction.auth_code, None, "{event}");
        assert_eq!(transaction.avs_response, None, "{event}");
    }
}

#[test]
fn authorization_carries_its_invoice_number() {
    let notification = notification("payment.authorization.created");
    let transaction = transaction(&notification);
    assert_eq!(transaction.invoice_number.as_deref(), Some("INV-1001"));
    assert_eq!(
        transaction.merchant_reference_id.as_deref(),
        Some("ORDER-1001")
    );
}

#[test]
fn fraud_events_list_the_filters() {
    let cases = [
        (
            "payment.fraud.held",
            EventType::FraudHeld,
            TransactionOutcome::HeldForReview,
        ),
        (
            "payment.fraud.approved",
            EventType::FraudApproved,
            TransactionOutcome::Approved,
        ),
        (
            "payment.fraud.declined",
            EventType::FraudDeclined,
            TransactionOutcome::Declined,
        ),
    ];
    for (event, event_type, outcome) in cases {
        let notification = notification(event);
        assert_eq!(notification.event_type, event_type, "{event}");
        let transaction = transaction(&notification);
        assert_eq!(transaction.response_code, Some(outcome), "{event}");
        assert_eq!(transaction.auth_amount, Some(dec("50000")), "{event}");
        let filters: Vec<_> = transaction
            .fraud_list
            .iter()
            .map(|f| (f.fraud_filter.as_str(), f.fraud_action.as_str()))
            .collect();
        assert_eq!(filters, [("AmountFilter", "authAndHold")], "{event}");
    }
}

#[test]
fn customer_event_carries_the_profile() {
    let notification = notification("customer.created");
    assert_eq!(notification.event_type, EventType::CustomerCreated);
    let Payload::CustomerProfile(profile) = &notification.payload else {
        panic!(
            "expected a customer profile, got {:?}",
            notification.payload
        );
    };
    assert_eq!(profile.id, "394");
    assert_eq!(profile.merchant_customer_id.as_deref(), Some("cust457"));
    assert_eq!(
        profile.description.as_deref(),
        Some("Profile created by Subscription: 1447")
    );
    assert_eq!(profile.email, None);
    assert_eq!(profile.payment_profiles.len(), 1);
    assert_eq!(profile.payment_profiles[0].id, "694");
    assert_eq!(
        profile.payment_profiles[0].customer_type,
        Some(CustomerType::Individual)
    );
}

#[test]
fn payment_profile_event_reads_a_numeric_profile_id_as_text() {
    let notification = notification("customer.paymentProfile.created");
    assert_eq!(notification.event_type, EventType::PaymentProfileCreated);
    let Payload::CustomerPaymentProfile(profile) = &notification.payload else {
        panic!("expected a payment profile, got {:?}", notification.payload);
    };
    assert_eq!(profile.id, "694");
    assert_eq!(profile.customer_profile_id.as_deref(), Some("394"));
    assert_eq!(profile.customer_type, Some(CustomerType::Business));
}

#[test]
fn subscription_event_carries_the_subscription() {
    let notification = notification("customer.subscription.created");
    assert_eq!(notification.event_type, EventType::SubscriptionCreated);
    assert_eq!(
        notification.event_date,
        datetime!(2016-03-08 08:18:27.434 UTC)
    );
    let Payload::Subscription(subscription) = &notification.payload else {
        panic!("expected a subscription, got {:?}", notification.payload);
    };
    assert_eq!(subscription.id, "1405");
    assert_eq!(subscription.name.as_deref(), Some("testSubscription"));
    assert_eq!(subscription.amount, Some(dec("23")));
    assert_eq!(subscription.status, Some(ArbSubscriptionStatus::Active));
    let profile = subscription.profile.as_ref().expect("profile");
    assert_eq!(profile.customer_profile_id.as_deref(), Some("348"));
    assert_eq!(profile.customer_payment_profile_id.as_deref(), Some("644"));
    assert_eq!(profile.customer_shipping_address_id.as_deref(), Some("675"));
}

#[test]
fn other_entities_keep_their_json() {
    let notification = notification("partner.boarding.merchantAccount.created");
    assert_eq!(
        notification.event_type,
        EventType::BoardingMerchantAccountCreated
    );
    let Payload::Other(fields) = &notification.payload else {
        panic!("expected another entity, got {:?}", notification.payload);
    };
    assert_eq!(fields["entityName"], "boardingApplication");
    assert_eq!(fields["gatewayID"], "900057");
}

#[test]
fn unknown_event_types_and_fields_still_parse() {
    let notification = notification("unknown.event");
    assert_eq!(
        notification.event_type,
        EventType::Other("net.authorize.example.future.created".into())
    );
    assert_eq!(transaction(&notification).id, "60020981681");
}

#[test]
fn unknown_subscription_statuses_are_kept() {
    let body = br#"{"notificationId":"n","eventType":"net.authorize.customer.subscription.cancelled","eventDate":"2026-11-02T15:04:05Z","webhookId":"w","payload":{"entityName":"subscription","id":"1","status":"cancelled"}}"#;
    let notification = parse(body).unwrap();
    assert_eq!(notification.event_type, EventType::SubscriptionCancelled);
    let Payload::Subscription(subscription) = &notification.payload else {
        panic!("expected a subscription, got {:?}", notification.payload);
    };
    assert_eq!(
        subscription.status,
        Some(ArbSubscriptionStatus::Other("cancelled".into()))
    );
}

#[test]
fn event_types_display_as_their_wire_names() {
    let notification = notification("customer.paymentProfile.created");
    assert_eq!(
        notification.event_type.as_str(),
        "net.authorize.customer.paymentProfile.created"
    );
    assert_eq!(
        EventType::PriorAuthCaptureCreated.to_string(),
        "net.authorize.payment.priorAuthCapture.created"
    );
    let other = EventType::Other("net.authorize.example.future.created".into());
    assert_eq!(other.as_str(), "net.authorize.example.future.created");
}

#[test]
fn rejects_malformed_json() {
    assert!(parse(b"{not json").is_err());
}

#[test]
fn rejects_a_missing_required_field() {
    let body = br#"{"eventType":"net.authorize.payment.void.created","eventDate":"2026-11-02T15:04:05Z","webhookId":"w","payload":{"entityName":"transaction","id":"1"}}"#;
    let err = parse(body).unwrap_err();
    assert!(err.to_string().contains("notificationId"), "{err}");
}

#[test]
fn rejects_a_transaction_without_an_id() {
    let body = br#"{"notificationId":"n","eventType":"net.authorize.payment.void.created","eventDate":"2026-11-02T15:04:05Z","webhookId":"w","payload":{"entityName":"transaction"}}"#;
    assert!(parse(body).is_err());
}

#[test]
fn rejects_an_invalid_date() {
    let body = br#"{"notificationId":"n","eventType":"net.authorize.payment.void.created","eventDate":"yesterday","webhookId":"w","payload":{"entityName":"transaction","id":"1"}}"#;
    assert!(parse(body).is_err());
}
