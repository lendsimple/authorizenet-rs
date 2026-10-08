//! What the async and blocking clients share: settings, logging, and the calls into
//! the sans-IO protocol.

use std::time::Duration;

use crate::api::ApiRequest;
use crate::credentials::{Credentials, Environment};
use crate::error::Error;
use crate::protocol;
use crate::schema::SENSITIVE_ELEMENTS;

/// How long a request may take, including reading the response, unless set otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// The `User-Agent` sent unless set otherwise.
pub const DEFAULT_USER_AGENT: &str = concat!("authorizenet-rs/", env!("CARGO_PKG_VERSION"));

/// A client's settings.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub credentials: Credentials,
    pub endpoint: String,
    pub timeout: Duration,
    pub user_agent: String,
    pub validate_requests: bool,
}

impl Config {
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials,
            endpoint: Environment::default().endpoint().to_owned(),
            timeout: DEFAULT_TIMEOUT,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            validate_requests: true,
        }
    }

    /// The request body, validated unless validation is turned off.
    pub fn encode<R: ApiRequest>(&self, request: &R) -> Result<String, Error> {
        let body = if self.validate_requests {
            protocol::encode(request, &self.credentials)?
        } else {
            protocol::encode_unvalidated(request, &self.credentials)?
        };
        tracing::debug!(endpoint = %self.endpoint, "sending request");
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(body = %redact(&body), "request body");
        }
        Ok(body)
    }

    pub fn decode<R: ApiRequest>(&self, status: u16, body: &[u8]) -> Result<R::Response, Error> {
        tracing::debug!(status, "received response");
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(body = %redact(&String::from_utf8_lossy(body)), "response body");
        }
        let result = protocol::decode::<R::Response>(status, body);
        if let Err(err) = &result {
            tracing::debug!(error = %err, "request failed");
        }
        result
    }
}

/// The span every request runs in.
pub(crate) fn request_span<R: ApiRequest>() -> tracing::Span {
    tracing::debug_span!("authorizenet", operation = R::OPERATION)
}

/// Replaces the content of every sensitive element (card numbers, keys, tokens, ...)
/// with `[REDACTED]`, for logging.
pub(crate) fn redact(xml: &str) -> String {
    let mut out = xml.to_owned();
    for name in SENSITIVE_ELEMENTS {
        let open = format!("<{name}>");
        let close = format!("</{name}>");
        let mut from = 0;
        while let Some(start) = out[from..].find(&open).map(|i| from + i + open.len()) {
            let Some(end) = out[start..].find(&close).map(|i| start + i) else {
                // A truncated body: redact everything after the opening tag.
                out.replace_range(start.., "[REDACTED]");
                break;
            };
            out.replace_range(start..end, "[REDACTED]");
            from = start + "[REDACTED]".len() + close.len();
        }
    }
    out
}

/// The builder settings both clients have, as methods on `$builder`, whose `config`
/// field is a [`Config`].
macro_rules! common_builder_methods {
    () => {
        /// Sends requests to `environment`. The default is the sandbox.
        pub fn environment(mut self, environment: $crate::Environment) -> Self {
            self.config.endpoint = environment.endpoint().to_owned();
            self
        }

        /// How long a request may take in total. The default is 60 seconds.
        pub fn timeout(mut self, timeout: ::std::time::Duration) -> Self {
            self.config.timeout = timeout;
            self
        }

        /// The `User-Agent` header. The default is `authorizenet-rs/<version>`.
        pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
            self.config.user_agent = user_agent.into();
            self
        }

        /// Whether to check requests against the schema's facets before sending them
        /// (the default), failing with [`Error::Validation`](crate::Error::Validation).
        pub fn validate_requests(mut self, validate: bool) -> Self {
            self.config.validate_requests = validate;
            self
        }
    };
}

pub(crate) use common_builder_methods;

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_every_sensitive_element() {
        let xml = "<a><cardNumber>4111111111111111</cardNumber><cardCode>123</cardCode>\
                   <name>Ellen</name><cardNumber>5424000000000015</cardNumber></a>";
        assert_eq!(
            redact(xml),
            "<a><cardNumber>[REDACTED]</cardNumber><cardCode>[REDACTED]</cardCode>\
             <name>Ellen</name><cardNumber>[REDACTED]</cardNumber></a>"
        );
    }

    #[test]
    fn empty_element_is_unchanged() {
        assert_eq!(redact("<cardNumber/>"), "<cardNumber/>");
    }

    #[test]
    fn truncated_body_is_redacted_to_the_end() {
        assert_eq!(redact("<cardNumber>4111</card"), "<cardNumber>[REDACTED]");
    }
}
