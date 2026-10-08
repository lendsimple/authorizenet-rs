//! A typed client for the [Authorize.Net](https://developer.authorize.net/api/reference/)
//! XML API.
//!
//! Every request and response type is generated from Authorize.Net's XSD (see
//! [`schema`]), and every operation is a method on a resource of the client:
//! `client.transactions().create(&request)`.
//!
//! # Clients and transports
//!
//! [`Client`] is async and [`blocking::Client`] blocks. Both send requests through a
//! transport, which you can replace to use any HTTP client (see [`transport`]). The
//! features provide the default transports:
//!
//! - `reqwest` (default): `transport::ReqwestTransport` for [`Client`], on tokio.
//! - `ureq`: `blocking::UreqTransport` for [`blocking::Client`], with no async runtime.
//! - `rustls` (default) or `native-tls`: their TLS implementation.
//!
//! [`protocol`] also encodes requests and decodes responses on their own.
//!
//! # Logging
//!
//! The clients emit [`tracing`](https://docs.rs/tracing) events: each request runs in
//! an `authorizenet` span with its `operation`, at `DEBUG` level, and request and
//! response bodies are logged at `TRACE` level with card numbers, keys and other
//! secrets redacted.

// Lets derived code refer to `::authorizenet::...` inside this crate too.
extern crate self as authorizenet;

pub mod api;
pub mod blocking;
mod client;
mod config;
mod credentials;
mod error;
mod operations;
pub mod protocol;
pub mod schema;
pub mod transport;
pub mod types;
pub mod validate;
pub mod xml;

pub use api::{ApiRequest, ApiResponse};
pub use client::{Client, ClientBuilder};
pub use config::{DEFAULT_TIMEOUT, DEFAULT_USER_AGENT};
pub use credentials::{Credentials, Environment, PRODUCTION_ENDPOINT, SANDBOX_ENDPOINT};
pub use error::{ApiError, Error, TransportError};
pub use rust_decimal::Decimal;
pub use secrecy::SecretString;

/// Compiles the README's examples as doc tests.
#[cfg(all(doctest, feature = "reqwest", feature = "ureq"))]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
