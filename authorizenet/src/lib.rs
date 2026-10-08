//! A typed client for the [Authorize.Net](https://developer.authorize.net/api/reference/)
//! XML API.

// Lets derived code refer to `::authorizenet::...` inside this crate too.
extern crate self as authorizenet;

pub mod schema;
pub mod types;
pub mod xml;

pub use rust_decimal::Decimal;
