//! Derive macros for the [`authorizenet`](https://docs.rs/authorizenet) crate.
//!
//! These map Rust types onto the Authorize.Net XML schema. They are used by the
//! crate's generated schema types and are not intended as a general XML framework.
//!
//! # `#[derive(AnetXml)]`
//!
//! On a **struct** with named fields: an XSD complex type whose fields are written in
//! declaration order (the `xs:sequence` order) and read in any order.
//! The derive also implements `Debug` (so that `sensitive` fields can be redacted);
//! do not derive `Debug` separately.
//!
//! Container attributes:
//! - `root = "name"`: the type can be a whole document with this root element.
//! - `request`: the root is an `ANetApiRequest`; `merchantAuthentication` is supplied by
//!   the client when writing and ignored when reading.
//!
//! Field attributes (the field's type decides cardinality: `T` is required,
//! `Option<T>` is optional, `Vec<T>` repeats):
//! - `rename = "xmlName"`: element name (default: the field name in lowerCamelCase).
//! - `flatten`: the fields of an XSD base type, written inline.
//! - `choice`: the field's type is a choice enum; its variants are sibling elements.
//! - `wrapper, item = "child"`: a `Vec<T>` (or `Option<Vec<T>>`) inside a container element.
//!   An empty `Vec` omits the container unless `keep_empty` is also given.
//! - `attribute`: an XML attribute rather than a child element.
//! - `sensitive`: redacted in `Debug` output.
//!
//! On an **enum** whose variants each hold one value: an `xs:choice` group. Variant
//! attributes: `rename = "xmlName"` (default: the variant name in lowerCamelCase) and
//! `sensitive`.
//!
//! # `#[derive(AnetEnum)]`
//!
//! On an enum of unit variants, each with `#[anet(value = "xmlValue")]`. One variant may
//! be `#[anet(other)] Other(String)` to keep values that are not listed.

mod attrs;
mod cardinality;
mod naming;
mod xml_choice;
mod xml_enum;
mod xml_struct;

use proc_macro::TokenStream;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(AnetXml, attributes(anet))]
pub fn derive_anet_xml(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let result = if !input.generics.params.is_empty() {
        Err(syn::Error::new_spanned(
            &input.generics,
            "AnetXml does not support generic types",
        ))
    } else {
        match &input.data {
            Data::Struct(data) => xml_struct::expand(&input, data),
            Data::Enum(data) => xml_choice::expand(&input, data),
            Data::Union(_) => Err(syn::Error::new_spanned(
                &input.ident,
                "AnetXml cannot be derived for unions",
            )),
        }
    };
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

#[proc_macro_derive(AnetEnum, attributes(anet))]
pub fn derive_anet_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    xml_enum::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
