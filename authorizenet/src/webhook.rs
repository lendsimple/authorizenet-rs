//! Webhook notifications: checking that they came from Authorize.Net.
//!
//! Authorize.Net signs every notification it posts. The `X-ANET-Signature` header
//! holds `sha512=` and the HMAC-SHA512 of the request body, keyed with the merchant's
//! Signature Key (Merchant Interface: Account > Settings > Security Settings > API
//! Credentials and Keys). [`verify_signature`] recomputes it and compares the two.
//!
//! Pass the body exactly as it was received. A body parsed and serialized again, with
//! other whitespace or key order, no longer matches its signature.
//!
//! ```
//! use authorizenet::SecretString;
//! use authorizenet::webhook::{SignatureError, verify_signature};
//!
//! fn accept(key: &SecretString, header: Option<&str>, body: &[u8]) -> bool {
//!     let Some(header) = header else { return false };
//!     match verify_signature(key, header, body) {
//!         Ok(()) => true,
//!         Err(err) if err.is_malformed() => {
//!             eprintln!("malformed signature header: {err}");
//!             false
//!         }
//!         Err(err) => {
//!             eprintln!("rejected notification: {err}");
//!             false
//!         }
//!     }
//! }
//! # let key = SecretString::from("key");
//! # assert!(!accept(&key, Some("sha512=00"), b"{}"));
//! ```

use hmac::{Hmac, KeyInit, Mac};
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha512;

/// The header that carries a notification's signature.
pub const SIGNATURE_HEADER: &str = "X-ANET-Signature";

const PREFIX: &str = "sha512=";

/// Checks the `X-ANET-Signature` header of a notification against its raw body.
///
/// `signature_header` is the header's value, such as `sha512=6F2A…`. The `sha512=`
/// prefix may be in any case, and the digest in upper- or lowercase hex. The
/// Signature Key is used as given: its characters, not the bytes its hex digits
/// spell. The comparison takes constant time.
pub fn verify_signature(
    signature_key: &SecretString,
    signature_header: &str,
    body: &[u8],
) -> Result<(), SignatureError> {
    let header = signature_header.trim_ascii();
    let digest = match header.get(..PREFIX.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(PREFIX) => &header[PREFIX.len()..],
        _ => return Err(SignatureError::MissingPrefix),
    };
    let expected = decode_digest(digest).ok_or(SignatureError::InvalidDigest)?;

    let mut mac = Hmac::<Sha512>::new_from_slice(signature_key.expose_secret().as_bytes())
        .expect("HMAC takes keys of any length");
    mac.update(body);
    mac.verify_slice(&expected)
        .map_err(|_| SignatureError::Mismatch)
}

/// Decodes the 128 hex digits of a SHA-512 digest.
fn decode_digest(hex: &str) -> Option<[u8; 64]> {
    let (pairs, []) = hex.as_bytes().as_chunks::<2>() else {
        return None;
    };
    if pairs.len() != 64 {
        return None;
    }
    let mut digest = [0; 64];
    for (byte, &[high, low]) in digest.iter_mut().zip(pairs) {
        *byte = hex_value(high)? << 4 | hex_value(low)?;
    }
    Some(digest)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// Why a notification's signature was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SignatureError {
    /// The header does not start with `sha512=`.
    #[error("the X-ANET-Signature header does not start with `sha512=`")]
    MissingPrefix,

    /// The header's digest is not 128 hex digits.
    #[error("the X-ANET-Signature header's digest is not 128 hex digits")]
    InvalidDigest,

    /// The signature does not match the body: the notification was altered, was not
    /// sent by Authorize.Net, or was signed with another Signature Key.
    #[error("the X-ANET-Signature does not match the body")]
    Mismatch,
}

impl SignatureError {
    /// Whether the header itself is malformed, rather than a well-formed signature
    /// that does not match.
    pub fn is_malformed(&self) -> bool {
        matches!(
            self,
            SignatureError::MissingPrefix | SignatureError::InvalidDigest
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The sample notification from Authorize.Net's webhook documentation.
    const BODY: &[u8] = br#"{"notificationId":"d0e8e7fe-c3e7-4add-a480-27bc5ce28a18","eventType":"net.authorize.payment.authcapture.created","eventDate":"2017-03-29T20:48:02.0080095Z","webhookId":"63d6fea2-aa13-4b1d-a204-f5fbc15942b7","payload":{"responseCode":1,"merchantReferenceId":"19102146534003137356","authCode":"LZ6I19","avsResponse":"Y","authAmount":45.00,"entityName":"transaction","id":"60020981676"}}"#;

    const KEY: &str = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\
                       0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

    // HMAC-SHA512 of BODY keyed with KEY's characters, computed with
    // `openssl dgst -sha512 -hmac "$KEY"` and with Python's `hmac`.
    const SIGNATURE: &str = "BB7CAB5B8DBADF5787582BCC32FFBE05D2403E6C88C48F9289963D52A0289824\
                             0557DBA374CA52C830A56FE78272998C9717742F6E48E32425B645BA9CFAB4E9";

    // The same HMAC keyed with the 64 bytes KEY's hex digits spell.
    const SIGNATURE_WITH_DECODED_KEY: &str = "0BAD3C94F6A64F4317057B13706C171EB5920DFFC772895625000A19788896A1\
         6CB421D56983AC62B1B8F03273F024874D76BAD41A28E5B1AD9F759CD84D1A7C";

    fn verify(header: &str, body: &[u8]) -> Result<(), SignatureError> {
        verify_signature(&SecretString::from(KEY), header, body)
    }

    #[test]
    fn accepts_a_valid_signature() {
        assert_eq!(verify(&format!("sha512={SIGNATURE}"), BODY), Ok(()));
    }

    #[test]
    fn accepts_lowercase_hex() {
        let header = format!("sha512={}", SIGNATURE.to_ascii_lowercase());
        assert_eq!(verify(&header, BODY), Ok(()));
    }

    #[test]
    fn accepts_the_prefix_in_any_case() {
        assert_eq!(verify(&format!("SHA512={SIGNATURE}"), BODY), Ok(()));
    }

    #[test]
    fn ignores_surrounding_whitespace() {
        assert_eq!(verify(&format!(" sha512={SIGNATURE}\r\n"), BODY), Ok(()));
    }

    #[test]
    fn rejects_a_changed_body() {
        let mut body = BODY.to_vec();
        let last = body.len() - 3;
        body[last] ^= 1;
        assert_eq!(
            verify(&format!("sha512={SIGNATURE}"), &body),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn rejects_another_key() {
        let key = SecretString::from(KEY.replace('0', "1"));
        assert_eq!(
            verify_signature(&key, &format!("sha512={SIGNATURE}"), BODY),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn keys_with_the_key_characters_not_its_decoded_hex() {
        assert_eq!(
            verify(&format!("sha512={SIGNATURE_WITH_DECODED_KEY}"), BODY),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn rejects_a_missing_prefix() {
        assert_eq!(verify(SIGNATURE, BODY), Err(SignatureError::MissingPrefix));
    }

    #[test]
    fn rejects_another_algorithm() {
        assert_eq!(
            verify(&format!("sha256={SIGNATURE}"), BODY),
            Err(SignatureError::MissingPrefix)
        );
    }

    #[test]
    fn rejects_an_empty_header() {
        assert_eq!(verify("", BODY), Err(SignatureError::MissingPrefix));
    }

    #[test]
    fn rejects_odd_length_hex() {
        assert_eq!(
            verify(&format!("sha512={SIGNATURE}0"), BODY),
            Err(SignatureError::InvalidDigest)
        );
    }

    #[test]
    fn rejects_a_non_hex_digit() {
        let header = format!("sha512=G{}", &SIGNATURE[1..]);
        assert_eq!(verify(&header, BODY), Err(SignatureError::InvalidDigest));
    }

    #[test]
    fn rejects_a_short_digest() {
        let header = format!("sha512={}", &SIGNATURE[..126]);
        assert_eq!(verify(&header, BODY), Err(SignatureError::InvalidDigest));
    }

    #[test]
    fn rejects_an_empty_digest() {
        assert_eq!(verify("sha512=", BODY), Err(SignatureError::InvalidDigest));
    }

    #[test]
    fn rejects_non_ascii_without_panicking() {
        // The prefix length falls inside a multibyte character.
        assert_eq!(
            verify("sha512é00", BODY),
            Err(SignatureError::MissingPrefix)
        );
        let header = format!("sha512=é{}", &SIGNATURE[2..]);
        assert_eq!(verify(&header, BODY), Err(SignatureError::InvalidDigest));
    }

    #[test]
    fn malformed_headers_are_told_apart_from_mismatches() {
        assert!(SignatureError::MissingPrefix.is_malformed());
        assert!(SignatureError::InvalidDigest.is_malformed());
        assert!(!SignatureError::Mismatch.is_malformed());
    }
}
