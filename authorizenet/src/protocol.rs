//! Encoding requests and decoding responses, without any I/O.
//!
//! The clients are thin layers over these two functions, which can also drive any
//! other HTTP client: POST the encoded body to [`Environment::endpoint`] with
//! `Content-Type: application/xml`, then decode the status and body.
//!
//! [`Environment::endpoint`]: crate::Environment::endpoint

use crate::api::{ApiRequest, ApiResponse};
use crate::credentials::Credentials;
use crate::error::{ApiError, Error};
use crate::schema::{ErrorResponse, MessageType, Messages};
use crate::xml::{self, XmlRoot};

/// The `Content-Type` of request bodies.
pub const CONTENT_TYPE: &str = "application/xml";

/// Validates `request`, then writes it as a document, adding `credentials` as its
/// `merchantAuthentication` (except for unauthenticated requests such as `isAlive`).
pub fn encode<R: ApiRequest>(request: &R, credentials: &Credentials) -> Result<String, Error> {
    request.validate()?;
    encode_unvalidated(request, credentials)
}

/// Like [`encode`], without validating the request first.
pub fn encode_unvalidated<R: ApiRequest>(
    request: &R,
    credentials: &Credentials,
) -> Result<String, Error> {
    let document = if R::AUTHENTICATED {
        xml::to_string_with_auth(request, &credentials.to_merchant_authentication())?
    } else {
        xml::to_string(request)?
    };
    Ok(document)
}

/// Reads the response to a request from its HTTP status and body.
///
/// Fails with [`Error::Status`] for a non-2xx status, and with [`Error::Api`] when the
/// API reports result code `Error`: in an `ErrorResponse`, in the expected response
/// (whose content is then kept), or in a response that cannot be read as the expected
/// type, as happens when an error leaves out fields the schema requires.
pub fn decode<T: ApiResponse>(status: u16, body: &[u8]) -> Result<T, Error> {
    if !(200..300).contains(&status) {
        return Err(Error::Status {
            status,
            body: String::from_utf8_lossy(body).into_owned(),
        });
    }
    if xml::root_name(body)? == ErrorResponse::ROOT {
        let response: ErrorResponse = xml::from_slice(body)?;
        return Err(ApiError::new(response.messages.clone(), Some(response.into())).into());
    }
    match xml::from_slice::<T>(body) {
        Ok(response) if is_error(response.messages()) => {
            let messages = response.messages().clone();
            Err(ApiError::new(messages, Some(response.into())).into())
        }
        Ok(response) => Ok(response),
        Err(err) => match xml::from_slice_as::<ErrorResponse>(body) {
            // `ErrorResponse` holds just the fields every response starts with.
            Ok(base) if is_error(&base.messages) => Err(ApiError::new(base.messages, None).into()),
            _ => Err(err.into()),
        },
    }
}

fn is_error(messages: &Messages) -> bool {
    messages.result_code == MessageType::Error
}
