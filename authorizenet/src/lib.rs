//! A typed client for the [Authorize.Net](https://developer.authorize.net/api/reference/)
//! XML API.
//!
//! Every request and response type is generated from Authorize.Net's XSD (see
//! [`schema`]), and every operation is a method on a resource of the client:
//! `client.transactions().create(&request)`.
//!
//! # Features
//!
//! - `async` (default): [`Client`], built on reqwest, for tokio.
//! - `blocking`: [`blocking::Client`], built on ureq, with no async runtime.
//! - `rustls` (default) or `native-tls`: the TLS implementation.
//!
//! Without a client feature, [`protocol`] still encodes requests and decodes
//! responses for use with any HTTP client.
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
#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "async")]
mod client;
#[cfg(any(feature = "async", feature = "blocking"))]
mod config;
mod credentials;
mod error;
mod operations;
pub mod protocol;
pub mod schema;
pub mod types;
pub mod validate;
pub mod xml;

pub use api::{ApiRequest, ApiResponse};
#[cfg(feature = "async")]
pub use client::{Client, ClientBuilder};
#[cfg(any(feature = "async", feature = "blocking"))]
pub use config::{DEFAULT_TIMEOUT, DEFAULT_USER_AGENT};
pub use credentials::{Credentials, Environment, PRODUCTION_ENDPOINT, SANDBOX_ENDPOINT};
pub use error::{ApiError, Error, TransportError};
pub use rust_decimal::Decimal;
pub use secrecy::SecretString;

/// Compiles the README's examples as doc tests.
#[cfg(all(doctest, feature = "async", feature = "blocking"))]
#[doc = include_str!("../../README.md")]
struct ReadmeDoctests;
