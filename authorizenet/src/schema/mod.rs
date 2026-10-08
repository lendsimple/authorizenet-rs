//! Authorize.Net schema types, generated from `schema/AnetApiSchema.xsd`.
//!
//! Regenerate with `cargo xtask codegen` after changing the XSD or
//! `schema/overrides.toml`. Shortcuts for common requests, such as
//! [`TransactionRequest::auth_capture`] and [`CreditCard::new`], are written by hand.

// Choice enums mirror the XSD; boxing their large variants would only make
// requests clumsier to build.
#[rustfmt::skip]
mod enums;
#[rustfmt::skip]
#[allow(clippy::large_enum_variant)]
mod messages;
#[rustfmt::skip]
#[allow(clippy::large_enum_variant)]
mod types;

mod ext;

pub use enums::*;
pub use ext::TransactionOutcome;
pub use messages::*;
pub use types::*;
