//! Webhook notifications: checking that they came from Authorize.Net, and reading
//! them as typed values.
//!
//! Authorize.Net signs every notification it posts. The `X-ANET-Signature` header
//! holds `sha512=` and the HMAC-SHA512 of the request body, keyed with the merchant's
//! Signature Key (Merchant Interface: Account > Settings > Security Settings > API
//! Credentials and Keys). [`verify_signature`] recomputes it and compares the two;
//! [`parse`] reads the JSON body into a [`Notification`]; [`verify_then_parse`] does
//! both, in that order.
//!
//! Pass the body exactly as it was received. A body parsed and serialized again, with
//! other whitespace or key order, no longer matches its signature.
//!
//! ```
//! use authorizenet::SecretString;
//! use authorizenet::webhook::{EventType, Payload, WebhookError, verify_then_parse};
//!
//! fn handle(key: &SecretString, signature: &str, body: &[u8]) -> Result<(), WebhookError> {
//!     let notification = verify_then_parse(key, signature, body)?;
//!     match (&notification.event_type, &notification.payload) {
//!         (EventType::AuthCaptureCreated, Payload::Transaction(transaction)) => {
//!             println!("charged {:?} in {}", transaction.auth_amount, transaction.id);
//!         }
//!         (EventType::SubscriptionSuspended, Payload::Subscription(subscription)) => {
//!             println!("subscription {} suspended", subscription.id);
//!         }
//!         _ => {}
//!     }
//!     Ok(())
//! }
//! # let key = SecretString::from("key");
//! # assert!(matches!(handle(&key, "sha512=00", b"{}"), Err(WebhookError::Signature(_))));
//! ```
//!
//! Authorize.Net adds events and fields over time: an event this release does not
//! list parses as [`EventType::Other`], an entity as [`Payload::Other`], and unknown
//! fields are ignored.
//!
//! Settlements and returned eCheck payments have no webhook events. Their results
//! still come from the reporting API: `getTransactionDetails` reports a transaction's
//! `returnedItems`.

mod notification;
mod signature;

use secrecy::SecretString;

pub use notification::{
    CustomerPaymentProfilePayload, CustomerProfilePayload, EventType, FraudFilter, Notification,
    ParseError, Payload, PaymentProfileSummary, SubscriptionPayload, SubscriptionProfile,
    TransactionPayload, parse,
};
pub use signature::{SIGNATURE_HEADER, SignatureError, verify_signature};

/// Checks a notification's signature, then reads its body. The body is not parsed
/// unless the signature matches.
///
/// See [`verify_signature`] and [`parse`].
pub fn verify_then_parse(
    signature_key: &SecretString,
    signature_header: &str,
    body: &[u8],
) -> Result<Notification, WebhookError> {
    verify_signature(signature_key, signature_header, body)?;
    Ok(parse(body)?)
}

/// Why [`verify_then_parse`] rejected a notification.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WebhookError {
    /// The signature is malformed or does not match the body.
    #[error(transparent)]
    Signature(#[from] SignatureError),

    /// The signature matches, but the body is not a notification.
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[cfg(test)]
mod tests {
    use hmac::{Hmac, KeyInit, Mac};
    use secrecy::ExposeSecret;
    use sha2::Sha512;

    use super::*;

    const BODY: &[u8] = br#"{"notificationId":"n-1","eventType":"net.authorize.payment.void.created","eventDate":"2026-11-02T10:00:00Z","webhookId":"w-1","payload":{"entityName":"transaction","id":"60020981676","responseCode":1}}"#;

    fn key() -> SecretString {
        SecretString::from("signature-key")
    }

    fn sign(body: &[u8]) -> String {
        let mut mac = Hmac::<Sha512>::new_from_slice(key().expose_secret().as_bytes()).unwrap();
        mac.update(body);
        let digest = mac.finalize().into_bytes();
        let hex: String = digest.iter().map(|byte| format!("{byte:02X}")).collect();
        format!("sha512={hex}")
    }

    #[test]
    fn parses_a_verified_notification() {
        let notification = verify_then_parse(&key(), &sign(BODY), BODY).unwrap();
        assert_eq!(notification.event_type, EventType::VoidCreated);
    }

    #[test]
    fn rejects_a_bad_signature_before_parsing() {
        let other = sign(b"another body");
        assert!(matches!(
            verify_then_parse(&key(), &other, BODY),
            Err(WebhookError::Signature(SignatureError::Mismatch))
        ));
    }

    #[test]
    fn reports_a_signed_body_that_is_not_a_notification() {
        let body = br#"{"hello":"world"}"#;
        assert!(matches!(
            verify_then_parse(&key(), &sign(body), body),
            Err(WebhookError::Parse(_))
        ));
    }
}
