//! Errors returned by the client.

use std::fmt;

use crate::api::ApiResponse;
use crate::schema::{AnyResponse, Message, Messages};
use crate::validate::ValidationError;
use crate::xml::XmlError;

/// Why a request failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API processed the request and answered with result code `Error`.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request breaks the schema, so it was not sent.
    #[error("invalid request: {0}")]
    Validation(#[from] ValidationError),

    /// A request could not be written, or a response could not be read.
    #[error(transparent)]
    Xml(#[from] XmlError),

    /// The API answered with an HTTP status other than 2xx.
    #[error("HTTP status {status}")]
    Status {
        status: u16,
        /// The response body, which may help diagnose the failure.
        body: String,
    },
}

impl Error {
    /// The API error, if the API reported one.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(err: ApiError) -> Self {
        Error::Api(Box::new(err))
    }
}

/// An error the API reported: result code `Error`, with codes such as `E00027`.
///
/// Displays as `[code] text` of the first message. Some failures still carry a full
/// response; a declined transaction, for example, explains the decline in its
/// `transactionResponse`:
///
/// ```no_run
/// # fn handle(err: authorizenet::ApiError) {
/// use authorizenet::schema::CreateTransactionResponse;
///
/// if let Some(response) = err.response::<CreateTransactionResponse>() {
///     for e in &response.transaction_response.errors {
///         println!("{:?}: {:?}", e.error_code, e.error_text);
///     }
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    messages: Messages,
    response: Option<AnyResponse>,
}

impl ApiError {
    pub(crate) fn new(messages: Messages, response: Option<AnyResponse>) -> Self {
        Self { messages, response }
    }

    fn first(&self) -> Option<&Message> {
        self.messages.message.first()
    }

    /// The first message's code, such as `E00035`; empty if there is none.
    pub fn code(&self) -> &str {
        self.first().map_or("", |m| m.code.as_str())
    }

    /// The first message's text.
    pub fn text(&self) -> &str {
        self.first().map_or("", |m| m.text.as_str())
    }

    /// All the messages.
    pub fn messages(&self) -> &Messages {
        &self.messages
    }

    /// The response that reported the error, if it could be read.
    pub fn any_response(&self) -> Option<&AnyResponse> {
        self.response.as_ref()
    }

    /// The response that reported the error, if it was a `T`.
    pub fn response<T: ApiResponse>(&self) -> Option<&T> {
        self.response.as_ref().and_then(T::from_any)
    }

    /// Takes the response that reported the error.
    pub fn into_response(self) -> Option<AnyResponse> {
        self.response
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code(), self.text())
    }
}

impl std::error::Error for ApiError {}
