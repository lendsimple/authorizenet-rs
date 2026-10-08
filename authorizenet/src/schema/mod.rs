//! Authorize.Net schema types, generated from `schema/AnetApiSchema.xsd`.
//!
//! Regenerate with `cargo xtask codegen` after changing the XSD or
//! `schema/overrides.toml`.

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

pub use enums::*;
pub use messages::*;
pub use types::*;
