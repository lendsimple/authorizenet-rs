//! A typed client for the [Authorize.Net](https://developer.authorize.net/api/reference/)
//! XML API.

// Lets derived code refer to `::authorizenet::...` inside this crate too.
extern crate self as authorizenet;

pub mod api;
mod credentials;
mod error;
mod operations;
pub mod protocol;
pub mod schema;
pub mod types;
pub mod validate;
pub mod xml;

pub use api::{ApiRequest, ApiResponse};
pub use credentials::{Credentials, Environment, PRODUCTION_ENDPOINT, SANDBOX_ENDPOINT};
pub use error::{ApiError, Error};
pub use rust_decimal::Decimal;
pub use secrecy::SecretString;
