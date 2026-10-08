//! How the clients send requests over HTTP.
//!
//! A transport posts one request body and returns the response's status and body;
//! everything else (encoding, validation, decoding, logging) stays in the client. The
//! async [`Client`](crate::Client) uses a [`Transport`], by default
//! `ReqwestTransport` (with the `reqwest` feature); the [`blocking::Client`](crate::blocking::Client) uses a
//! [`blocking::Transport`](crate::blocking::Transport), by default `UreqTransport`.
//!
//! The clients work on WebAssembly too: `ReqwestTransport` uses the JavaScript
//! `fetch` API there, and on `wasm32` the trait does not require `Send`.
//!
//! To use another HTTP client, implement the trait:
//!
//! ```
//! use authorizenet::TransportError;
//! use authorizenet::transport::{HttpRequest, HttpResponse, Transport};
//!
//! struct MyTransport;
//!
//! impl Transport for MyTransport {
//!     async fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
//!         // POST `request.body` to `request.url` with the `Content-Type` and
//!         // `User-Agent` headers given, within `request.timeout`.
//!         # let _ = request;
//!         # Ok(HttpResponse::new(200, Vec::new()))
//!     }
//! }
//! ```

use std::future::Future;
use std::time::Duration;

use crate::error::TransportError;

/// An HTTP POST to the API.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HttpRequest<'a> {
    /// The endpoint, from the client's [`Environment`](crate::Environment).
    pub url: &'a str,
    /// The XML request document.
    pub body: String,
    /// The `Content-Type` header: `application/xml`.
    pub content_type: &'static str,
    /// The `User-Agent` header.
    pub user_agent: &'a str,
    /// How long the request may take in total, including reading the response.
    pub timeout: Duration,
}

/// The API's answer to an [`HttpRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self { status, body }
    }
}

/// Sends requests for the async [`Client`](crate::Client).
///
/// The future must be `Send`, so that client calls can be spawned on a multi-threaded
/// runtime. (On WebAssembly, where there are no threads to send it to and `fetch`
/// futures are not `Send`, it need not be; see the crate's README.)
///
/// Return a [`TransportError`] only when no response was received (a connection, TLS
/// or timeout failure); an HTTP error status is a response.
#[cfg(not(target_arch = "wasm32"))]
pub trait Transport: Send + Sync {
    fn send(
        &self,
        request: HttpRequest<'_>,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;
}

/// Sends requests for the async [`Client`](crate::Client).
///
/// On WebAssembly the future need not be `Send`: there are no threads to send it to,
/// and JavaScript `fetch` futures are not `Send`.
///
/// Return a [`TransportError`] only when no response was received (a connection, TLS
/// or timeout failure); an HTTP error status is a response.
#[cfg(target_arch = "wasm32")]
pub trait Transport {
    fn send(
        &self,
        request: HttpRequest<'_>,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>>;
}

/// A [`Transport`] built on [reqwest](https://docs.rs/reqwest), which runs on tokio.
#[cfg(feature = "reqwest")]
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest")]
impl Default for ReqwestTransport {
    /// A transport with its own connection pool.
    ///
    /// # Panics
    ///
    /// Like `reqwest::Client::new`, if the TLS backend cannot be initialized.
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Client> for ReqwestTransport {
    /// Uses an existing reqwest client, for example to share its connection pool or
    /// proxy settings.
    fn from(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest")]
impl Transport for ReqwestTransport {
    async fn send(&self, request: HttpRequest<'_>) -> Result<HttpResponse, TransportError> {
        let error = |err: reqwest::Error| {
            if err.is_timeout() {
                TransportError::timeout(err)
            } else {
                TransportError::new(err)
            }
        };
        let response = self
            .client
            .post(request.url)
            .header(reqwest::header::CONTENT_TYPE, request.content_type)
            .header(reqwest::header::USER_AGENT, request.user_agent)
            .timeout(request.timeout)
            .body(request.body)
            .send()
            .await
            .map_err(error)?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(error)?;
        Ok(HttpResponse::new(status, body.to_vec()))
    }
}
